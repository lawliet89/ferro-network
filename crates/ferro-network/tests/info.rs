#![forbid(unsafe_code)]

//! `client.info()` against a mock console: the happy path, and a 401
//! carrying the spec's `Error Message` body, so request construction and
//! error mapping are exercised end to end.

use ferro_network::{ApiError, Error, NetworkClient};
use secrecy::SecretString;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const FIXTURE_OK: &str = include_str!("fixtures/info_ok.json");
const FIXTURE_UNAUTHORIZED: &str = include_str!("fixtures/info_unauthorized.json");

fn client_for(server: &MockServer) -> NetworkClient {
    NetworkClient::builder()
        .base_url(server.uri())
        .api_key(SecretString::from("test-key".to_string()))
        .build()
        .expect("client builds")
}

fn json_response(status: u16, body: &str) -> ResponseTemplate {
    ResponseTemplate::new(status)
        .set_body_string(body)
        .insert_header("content-type", "application/json")
}

#[tokio::test]
async fn info_sends_the_api_key_and_decodes_the_version() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/info"))
        .and(header("x-api-key", "test-key"))
        .respond_with(json_response(200, FIXTURE_OK))
        .expect(1)
        .mount(&server)
        .await;

    let info = client_for(&server).info().await.expect("info succeeds");
    assert_eq!(info.application_version, "10.0.160");
}

#[tokio::test]
async fn info_unauthorized_maps_to_api_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/info"))
        .respond_with(json_response(401, FIXTURE_UNAUTHORIZED))
        .expect(1)
        .mount(&server)
        .await;

    let err = client_for(&server)
        .info()
        .await
        .expect_err("401 is an error");
    let Error::Api(api) = err else {
        panic!("expected Error::Api, got {err:?}");
    };
    assert_eq!(
        api,
        ApiError {
            status: 401,
            code: "api.authentication.missing-credentials".into(),
            message: "Missing credentials".into(),
            request_id: Some("3fa85f64-5717-4562-b3fc-2c963f66afa6".into()),
        }
    );
}

#[tokio::test]
async fn info_schema_mismatch_is_a_json_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/info"))
        .respond_with(json_response(200, r#"{"version":"10.0.160"}"#))
        .mount(&server)
        .await;

    let err = client_for(&server)
        .info()
        .await
        .expect_err("missing field is an error");
    assert!(matches!(err, Error::Json(_)), "got {err:?}");
}
