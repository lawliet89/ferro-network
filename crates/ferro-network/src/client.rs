//! `NetworkClient` -- the user-facing async client.

use std::time::Duration;

use log::{debug, info};
use reqwest::header::HeaderMap;
use secrecy::SecretString;
use serde::de::DeserializeOwned;
use url::Url;

use crate::auth::{API_KEY_HEADER, ApiKey};
use crate::error::{Error, Result};
use crate::models::ApplicationInfo;

const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_TOTAL_TIMEOUT: Duration = Duration::from_secs(30);

/// Path prefix of the integration API on a UniFi OS console. Not in the
/// spec (its `servers` entry is just `/integration`).
const UNIFI_OS_PREFIX: &str = "/proxy/network/integration";

/// Controls how the TLS stack validates the console's certificate.
///
/// `Native` uses the platform trust store and is the safe default.
/// `Pinned` trusts only the supplied PEM certificate (typical for a
/// console's self-signed certificate, fetched once and saved). The
/// certificate must still match the host you connect to.
/// `AcceptInvalid` disables verification entirely and is only available
/// when the library is built with the `insecure-tls` feature.
#[derive(Default, Clone)]
pub enum TlsMode {
    #[default]
    Native,
    Pinned(Vec<u8>),
    #[cfg(feature = "insecure-tls")]
    AcceptInvalid,
}

impl TlsMode {
    /// Short label for logs. Never includes certificate contents.
    const fn label(&self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Pinned(_) => "pinned",
            #[cfg(feature = "insecure-tls")]
            Self::AcceptInvalid => "accept-invalid (insecure!)",
        }
    }
}

/// Async client for the UniFi Network local integration API.
///
/// Construct via [`NetworkClient::builder`]. The client is cheap to clone
/// (the underlying `reqwest::Client` is reference-counted), so share one
/// instance across tasks.
#[derive(Clone)]
pub struct NetworkClient {
    http: reqwest::Client,
    base_url: Url,
}

impl NetworkClient {
    /// Start a builder for a new client.
    pub fn builder() -> NetworkClientBuilder {
        NetworkClientBuilder::default()
    }

    /// The base URL every request path is joined onto, e.g.
    /// `https://unifi.local/proxy/network/integration/`.
    pub const fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// `GET /v1/info`: the running Network application version.
    ///
    /// # Errors
    /// Any [`Error`] variant; in practice [`Error::Http`] for network
    /// failures, [`Error::Api`] for a non-2xx response, [`Error::Json`]
    /// if the body does not match the schema.
    pub async fn info(&self) -> Result<ApplicationInfo> {
        let info: ApplicationInfo = self.get_json("/v1/info").await?;
        info!(
            "fetched application info: version {}",
            info.application_version
        );
        Ok(info)
    }

    pub(crate) async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        debug!("GET {path}");
        let response = self.http.get(self.url(path)?).send().await?;
        Self::json_response(response).await
    }

    fn url(&self, path: &str) -> Result<Url> {
        self.base_url
            .join(path.trim_start_matches('/'))
            .map_err(|e| Error::InvalidUrl(format!("{path}: {e}")))
    }

    async fn json_response<T: DeserializeOwned>(response: reqwest::Response) -> Result<T> {
        if response.status().is_success() {
            // Decode from bytes rather than `Response::json` so a schema
            // mismatch surfaces as `Error::Json` (with serde's path and
            // line), not as an opaque `reqwest` decode error.
            let body = response.bytes().await?;
            return Ok(serde_json::from_slice(&body)?);
        }
        Err(Error::from_response(response).await)
    }
}

/// Builder for [`NetworkClient`].
#[derive(Default)]
pub struct NetworkClientBuilder {
    host: Option<String>,
    base_url: Option<String>,
    api_key: Option<SecretString>,
    tls: TlsMode,
}

impl NetworkClientBuilder {
    /// Set the UniFi OS console hostname (or `host:port`), without a
    /// scheme. The base URL becomes
    /// `https://{host}/proxy/network/integration`. Mutually exclusive
    /// with [`Self::base_url`].
    #[must_use]
    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.host = Some(host.into());
        self
    }

    /// Set the entire base URL instead of a host: for a self-hosted
    /// Network application with a non-standard path, or a mock server in
    /// tests. Mutually exclusive with [`Self::host`].
    #[must_use]
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Set the API key. It is held in a [`SecretString`] for the lifetime
    /// of the builder and sent as a sensitive `X-API-Key` header.
    #[must_use]
    pub fn api_key(mut self, key: SecretString) -> Self {
        self.api_key = Some(key);
        self
    }

    /// Choose the TLS validation mode. Defaults to [`TlsMode::Native`].
    #[must_use]
    pub fn tls(mut self, mode: TlsMode) -> Self {
        self.tls = mode;
        self
    }

    /// Finalise the builder.
    ///
    /// # Errors
    /// - [`Error::MissingApiKey`] when no API key was supplied.
    /// - [`Error::InvalidUrl`] when neither or both of `host` and
    ///   `base_url` were set, or the resulting URL does not parse.
    /// - [`Error::Other`] for an API key that is not a valid header value
    ///   or an unparseable pinned certificate.
    /// - [`Error::Http`] for `reqwest` client construction failures.
    pub fn build(self) -> Result<NetworkClient> {
        let api_key = self.api_key.ok_or(Error::MissingApiKey)?;
        let base_url = match (self.host, self.base_url) {
            (Some(host), None) => {
                // `https://https://x` and `https://x/y/proxy/...` both
                // parse as URLs, so a scheme or path has to be caught here.
                if host.is_empty() || host.contains('/') {
                    return Err(Error::InvalidUrl(format!(
                        "host {host:?} must be a hostname or host:port, without scheme or path"
                    )));
                }
                parse_base_url(&format!("https://{host}{UNIFI_OS_PREFIX}"))?
            }
            (None, Some(url)) => parse_base_url(&url)?,
            (None, None) => {
                return Err(Error::InvalidUrl(
                    "either host or base_url must be set".into(),
                ));
            }
            (Some(_), Some(_)) => {
                return Err(Error::InvalidUrl(
                    "host and base_url are mutually exclusive".into(),
                ));
            }
        };

        let header_value = ApiKey::new(api_key)
            .header_value()
            .map_err(|e| Error::Other(format!("API key is not a valid header value: {e}")))?;
        let mut headers = HeaderMap::new();
        headers.insert(API_KEY_HEADER, header_value);

        let tls_label = self.tls.label();
        let mut builder = reqwest::Client::builder()
            .default_headers(headers)
            .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
            .timeout(DEFAULT_TOTAL_TIMEOUT);
        builder = match self.tls {
            TlsMode::Native => builder,
            TlsMode::Pinned(pem) => {
                let cert = reqwest::Certificate::from_pem(&pem)
                    .map_err(|e| Error::Other(format!("invalid pinned certificate: {e}")))?;
                builder.tls_certs_only([cert])
            }
            #[cfg(feature = "insecure-tls")]
            TlsMode::AcceptInvalid => builder.tls_danger_accept_invalid_certs(true),
        };
        let http = builder.build()?;

        info!("NetworkClient built: base_url={base_url}, tls={tls_label}");
        debug!(
            "client timeouts: connect={DEFAULT_CONNECT_TIMEOUT:?}, total={DEFAULT_TOTAL_TIMEOUT:?}"
        );
        Ok(NetworkClient { http, base_url })
    }
}

/// Parse a base URL, adding the trailing slash that `Url::join` needs to
/// append (rather than replace) the last path segment.
fn parse_base_url(raw: &str) -> Result<Url> {
    let mut raw = raw.to_string();
    if !raw.ends_with('/') {
        raw.push('/');
    }
    let url = Url::parse(&raw).map_err(|e| Error::InvalidUrl(format!("{raw}: {e}")))?;
    match url.scheme() {
        "https" | "http" => Ok(url),
        other => Err(Error::InvalidUrl(format!(
            "{raw}: unsupported scheme {other:?}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use secrecy::SecretString;

    use super::NetworkClient;
    use crate::Error;

    fn key() -> SecretString {
        SecretString::from("test-key".to_string())
    }

    #[test]
    fn host_builds_the_unifi_os_base_url() {
        let client = NetworkClient::builder()
            .host("unifi.local:8443")
            .api_key(key())
            .build()
            .expect("builds");
        assert_eq!(
            client.base_url().as_str(),
            "https://unifi.local:8443/proxy/network/integration/"
        );
        assert_eq!(
            client.url("/v1/info").expect("joins").as_str(),
            "https://unifi.local:8443/proxy/network/integration/v1/info"
        );
    }

    #[test]
    fn base_url_is_used_verbatim_plus_trailing_slash() {
        let client = NetworkClient::builder()
            .base_url("http://127.0.0.1:9000/integration")
            .api_key(key())
            .build()
            .expect("builds");
        assert_eq!(
            client.url("v1/info").expect("joins").as_str(),
            "http://127.0.0.1:9000/integration/v1/info"
        );
    }

    #[test]
    fn host_and_base_url_conflict() {
        let err = NetworkClient::builder()
            .host("unifi.local")
            .base_url("https://unifi.local/x")
            .api_key(key())
            .build()
            .err()
            .expect("conflict is rejected");
        assert!(matches!(err, Error::InvalidUrl(msg) if msg.contains("mutually exclusive")));
    }

    #[test]
    fn missing_api_key_is_rejected() {
        let err = NetworkClient::builder()
            .host("unifi.local")
            .build()
            .err()
            .expect("missing key is rejected");
        assert!(matches!(err, Error::MissingApiKey));
    }

    #[test]
    fn host_with_scheme_is_rejected() {
        let err = NetworkClient::builder()
            .host("https://unifi.local")
            .api_key(key())
            .build()
            .err()
            .expect("scheme in host is rejected");
        assert!(
            matches!(&err, Error::InvalidUrl(msg) if msg.contains("without scheme or path")),
            "got {err:?}"
        );
    }
}
