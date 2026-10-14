#![forbid(unsafe_code)]

//! Logging wiring: the filter precedence (`--log-level` >
//! `UNIFI_NETWORK_LOG` > `RUST_LOG` > `warn`) and that logs go to stderr.

mod common;

use predicates::prelude::*;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// The `info!` the library emits after a successful `info()`.
const INFO_LINE: &str = "fetched application info";

/// Run `info` against a fresh mock with extra env vars and flags.
async fn run_info(
    env: &'static [(&'static str, &'static str)],
    flags: &'static [&'static str],
) -> assert_cmd::assert::Assert {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/info"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"applicationVersion":"10.0.160"}"#)
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;
    let base_url = server.uri();
    tokio::task::spawn_blocking(move || {
        let key = common::key_file("test-key");
        common::isolated_cmd()
            .envs(env.iter().copied())
            .arg("--base-url")
            .arg(&base_url)
            .arg("--api-key-file")
            .arg(key.path())
            .args(flags)
            .arg("info")
            .assert()
    })
    .await
    .expect("spawn_blocking")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn default_level_is_warn() {
    run_info(&[], &[])
        .await
        .success()
        .stderr(predicate::str::contains(INFO_LINE).not());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn flag_enables_info_on_stderr_only() {
    run_info(&[], &["--log-level", "info"])
        .await
        .success()
        .stderr(predicate::str::contains(INFO_LINE))
        .stdout(predicate::str::contains(INFO_LINE).not());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn debug_logs_request_breadcrumbs_but_never_the_key() {
    run_info(&[], &["--log-level", "trace"])
        .await
        .success()
        .stderr(predicate::str::contains("GET /v1/info"))
        .stderr(predicate::str::contains("test-key").not());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn app_env_var_sets_the_filter() {
    run_info(&[("UNIFI_NETWORK_LOG", "info")], &[])
        .await
        .success()
        .stderr(predicate::str::contains(INFO_LINE));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn app_env_var_beats_rust_log() {
    run_info(&[("UNIFI_NETWORK_LOG", "error"), ("RUST_LOG", "info")], &[])
        .await
        .success()
        .stderr(predicate::str::contains(INFO_LINE).not());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn flag_beats_env_vars() {
    run_info(&[("UNIFI_NETWORK_LOG", "debug")], &["--log-level", "error"])
        .await
        .success()
        .stderr(predicate::str::contains("DEBUG").not())
        .stderr(predicate::str::contains("INFO").not());
}
