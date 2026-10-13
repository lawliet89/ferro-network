//! Error type for the ferro-network client.

use std::fmt;

use log::warn;
use reqwest::StatusCode;
use thiserror::Error;

/// Convenience alias for `Result<T, ferro_network::Error>`.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors returned by [`NetworkClient`](crate::NetworkClient) operations.
#[derive(Debug, Error)]
pub enum Error {
    /// Transport-level HTTP failure: connection refused, TLS handshake
    /// error, timeout, etc. Carries the underlying `reqwest` error.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Failure from the middleware stack (retry policy exhausted, rate
    /// limiter, ...). Not produced until the middleware stack lands.
    #[error("HTTP middleware error: {0}")]
    Middleware(String),

    /// The server returned a non-2xx response. See [`ApiError`].
    #[error(transparent)]
    Api(ApiError),

    /// A response body did not match the expected JSON schema.
    #[error("Failed to deserialize response: {0}")]
    Json(#[from] serde_json::Error),

    /// A URL passed to or constructed by the client was not valid.
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    /// The builder was finalised without an API key.
    #[error("API key not provided")]
    MissingApiKey,

    /// Catch-all for anything that does not fit the variants above.
    /// Prefer adding a more specific variant when patterns emerge.
    #[error("{0}")]
    Other(String),
}

/// A non-2xx response, decoded from the API's `Error Message` body where
/// possible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    /// HTTP status code.
    pub status: u16,
    /// The API's symbolic error code (e.g.
    /// `api.authentication.missing-credentials`), falling back to its
    /// `statusName` (e.g. `UNAUTHORIZED`) when the body has no `code`,
    /// or `unknown` when the body is not an `Error Message` at all.
    pub code: String,
    /// Human-readable message. For a body that is not an `Error
    /// Message`, the (truncated) raw body.
    pub message: String,
    /// The server-assigned request ID. Worth quoting in bug reports,
    /// especially for 5xx responses.
    pub request_id: Option<String>,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "API error {} ({}): {}",
            self.status, self.code, self.message
        )?;
        match &self.request_id {
            Some(id) if self.status >= 500 => write!(f, " [request ID {id}]"),
            _ => Ok(()),
        }
    }
}

impl std::error::Error for ApiError {}

impl Error {
    /// Build an [`Error::Api`] from a non-2xx response, consuming its body.
    pub(crate) async fn from_response(response: reqwest::Response) -> Self {
        let status = response.status();
        match response.text().await {
            Ok(body) => Self::Api(ApiError::from_body(status, &body)),
            Err(e) => Self::Http(e),
        }
    }
}

impl ApiError {
    fn from_body(status: StatusCode, body: &str) -> Self {
        match serde_json::from_str::<ErrorMessage>(body) {
            Ok(parsed) => {
                let code = parsed
                    .code
                    .filter(|c| !c.is_empty())
                    .or(parsed.status_name)
                    .unwrap_or_else(|| "unknown".to_string());
                Self {
                    status: status.as_u16(),
                    code,
                    message: parsed.message.unwrap_or_else(|| "(no message)".into()),
                    request_id: parsed.request_id,
                }
            }
            Err(e) => {
                warn!("API error body (HTTP {status}) did not match `Error Message`: {e}");
                Self {
                    status: status.as_u16(),
                    code: "unknown".to_string(),
                    message: truncate_body(body),
                    request_id: None,
                }
            }
        }
    }
}

/// The spec's `Error Message` schema, decoded leniently: every field is
/// optional so a partial body still yields what it has. Hand-written
/// rather than generated because typify makes the spec's required fields
/// mandatory, and an error path must not fail on a missing one.
///
/// Also covers the Site Manager cloud connector's own errors (`{code,
/// httpStatusCode, message, traceId}`), whose `traceId` plays the role of
/// `requestId`.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErrorMessage {
    status_name: Option<String>,
    code: Option<String>,
    message: Option<String>,
    #[serde(alias = "traceId")]
    request_id: Option<String>,
}

fn truncate_body(body: &str) -> String {
    const LIMIT: usize = 512;
    match body.char_indices().nth(LIMIT) {
        Some((idx, _)) => format!("{}...", &body[..idx]),
        None => body.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;

    use super::{ApiError, truncate_body};

    #[test]
    fn error_message_body_is_decoded() {
        let err = ApiError::from_body(
            StatusCode::UNAUTHORIZED,
            r#"{"statusCode":401,"statusName":"UNAUTHORIZED","code":"api.authentication.missing-credentials","message":"Missing credentials","timestamp":"2024-11-27T08:13:46.966Z","requestPath":"/integration/v1/info","requestId":"3fa85f64-5717-4562-b3fc-2c963f66afa6"}"#,
        );
        assert_eq!(
            err,
            ApiError {
                status: 401,
                code: "api.authentication.missing-credentials".into(),
                message: "Missing credentials".into(),
                request_id: Some("3fa85f64-5717-4562-b3fc-2c963f66afa6".into()),
            }
        );
        // The request ID is only shown for server errors.
        assert_eq!(
            err.to_string(),
            "API error 401 (api.authentication.missing-credentials): Missing credentials"
        );
    }

    #[test]
    fn empty_code_falls_back_to_status_name() {
        let err = ApiError::from_body(
            StatusCode::INTERNAL_SERVER_ERROR,
            r#"{"statusName":"INTERNAL_SERVER_ERROR","code":"","message":"Boom","requestId":"abc"}"#,
        );
        assert_eq!(err.code, "INTERNAL_SERVER_ERROR");
        assert_eq!(
            err.to_string(),
            "API error 500 (INTERNAL_SERVER_ERROR): Boom [request ID abc]"
        );
    }

    #[test]
    fn cloud_connector_error_body_is_decoded() {
        let err = ApiError::from_body(
            StatusCode::REQUEST_TIMEOUT,
            r#"{"code":"DeviceTimeout","httpStatusCode":408,"message":"Device connection timeout","traceId":"a1b2c3"}"#,
        );
        assert_eq!(err.code, "DeviceTimeout");
        assert_eq!(err.message, "Device connection timeout");
        assert_eq!(err.request_id.as_deref(), Some("a1b2c3"));
    }

    #[test]
    fn non_json_body_is_kept_truncated() {
        let err = ApiError::from_body(StatusCode::BAD_GATEWAY, "<html>bad gateway</html>");
        assert_eq!(err.code, "unknown");
        assert_eq!(err.message, "<html>bad gateway</html>");
        assert_eq!(err.request_id, None);
    }

    #[test]
    fn truncation_respects_char_boundaries() {
        let long = "é".repeat(600);
        let out = truncate_body(&long);
        assert_eq!(out.chars().count(), 512 + 3);
        assert!(out.ends_with("..."));
    }
}
