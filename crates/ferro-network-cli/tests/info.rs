#![forbid(unsafe_code)]

//! End to end: run the `ferro-network info` binary against a wiremock
//! server and assert on exit status and output.

mod common;

use predicates::prelude::*;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const FIXTURE: &str = r#"{"applicationVersion":"10.0.160"}"#;
const UNAUTHORIZED: &str = r#"{"statusCode":401,"statusName":"UNAUTHORIZED","code":"api.authentication.missing-credentials","message":"Missing credentials","requestId":"3fa85f64-5717-4562-b3fc-2c963f66afa6"}"#;

async fn mock(status: u16, body: &'static str) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/info"))
        .and(header("x-api-key", "test-key"))
        .respond_with(
            ResponseTemplate::new(status)
                .set_body_string(body)
                .insert_header("content-type", "application/json"),
        )
        .expect(1)
        .mount(&server)
        .await;
    server
}

/// Run `ferro-network --base-url <server> --api-key-file <tmp> <args>`.
/// `assert_cmd` blocks, so it runs off the runtime hosting the mock.
async fn run(server: &MockServer, args: &'static [&'static str]) -> assert_cmd::assert::Assert {
    let base_url = server.uri();
    tokio::task::spawn_blocking(move || {
        let key = common::key_file("test-key");
        common::isolated_cmd()
            .arg("--base-url")
            .arg(&base_url)
            .arg("--api-key-file")
            .arg(key.path())
            .args(args)
            .assert()
    })
    .await
    .expect("spawn_blocking")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn info_prints_the_version() {
    let server = mock(200, FIXTURE).await;
    run(&server, &["info"])
        .await
        .success()
        .stdout("Network application version: 10.0.160\n");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn info_json_prints_the_object() {
    let server = mock(200, FIXTURE).await;
    let assert = run(&server, &["--json", "info"]).await.success();
    let stdout: serde_json::Value =
        serde_json::from_slice(&assert.get_output().stdout).expect("stdout is JSON");
    assert_eq!(
        stdout,
        serde_json::json!({ "applicationVersion": "10.0.160" })
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn info_api_error_fails_with_the_api_code() {
    let server = mock(401, UNAUTHORIZED).await;
    run(&server, &["info"])
        .await
        .failure()
        .stdout("")
        .stderr(predicate::str::contains(
            "API error 401 (api.authentication.missing-credentials): Missing credentials",
        ));
}

#[test]
fn info_without_api_key_file_fails_before_any_request() {
    common::isolated_cmd()
        .args(["--host", "unifi.invalid", "info"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--api-key-file is required"));
}

#[test]
fn info_with_empty_api_key_file_fails() {
    let key = common::key_file("");
    common::isolated_cmd()
        .args(["--host", "unifi.invalid", "--api-key-file"])
        .arg(key.path())
        .arg("info")
        .assert()
        .failure()
        .stderr(predicate::str::contains("is empty"));
}
