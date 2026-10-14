//! Shared helpers for live tests.
//!
//! Cargo compiles every `tests/*.rs` file as its own test binary, so a
//! file that wants these helpers declares `mod common;`.

use ferro_network::NetworkClient;
use secrecy::SecretString;

const HOST_ENV: &str = "UNIFI_NETWORK_HOST";
const BASE_URL_ENV: &str = "UNIFI_NETWORK_BASE_URL";
const KEY_FILE_ENV: &str = "UNIFI_NETWORK_API_KEY_FILE";
const KEY_ENV: &str = "UNIFI_NETWORK_API_KEY";
const INSECURE_ENV: &str = "UNIFI_NETWORK_INSECURE";
const ALLOW_MUTATIONS_ENV: &str = "UNIFI_NETWORK_ALLOW_MUTATIONS";

/// Build a [`NetworkClient`] from the `UNIFI_NETWORK_*` env vars.
///
/// The target is `UNIFI_NETWORK_HOST` (a UniFi OS console) or
/// `UNIFI_NETWORK_BASE_URL` (a full base URL: a self-hosted Network
/// application, or the Site Manager cloud connector).
///
/// - Returns `None` when neither is set; the caller returns early so the
///   test passes without touching the network.
/// - **Panics** if both are set, or if a target is set but neither
///   `_API_KEY_FILE` nor `_API_KEY` is. A half-configured live env is
///   almost always a mistake worth surfacing loudly.
/// - **Panics** if `_INSECURE` is truthy but the library was built
///   without the `insecure-tls` feature. `scripts/live-test` enables it.
pub fn live_client() -> Option<NetworkClient> {
    let builder = match (std::env::var(HOST_ENV), std::env::var(BASE_URL_ENV)) {
        (Ok(host), Err(_)) => NetworkClient::builder().host(host),
        (Err(_), Ok(url)) => NetworkClient::builder().base_url(url),
        (Err(_), Err(_)) => return None,
        (Ok(_), Ok(_)) => panic!("set only one of {HOST_ENV} and {BASE_URL_ENV}"),
    };

    let key = match (std::env::var(KEY_FILE_ENV), std::env::var(KEY_ENV)) {
        (Ok(path), _) => {
            let raw = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("reading {KEY_FILE_ENV}={path} failed: {e}"));
            SecretString::from(raw.trim().to_string())
        }
        (Err(_), Ok(raw)) => SecretString::from(raw),
        (Err(_), Err(_)) => panic!(
            "a live target is set but no key source: set {KEY_FILE_ENV} (preferred) or {KEY_ENV}"
        ),
    };

    let builder = builder.api_key(key);
    let insecure = parse_boolish_env(INSECURE_ENV);
    #[cfg(not(feature = "insecure-tls"))]
    assert!(
        !insecure,
        "{INSECURE_ENV} is set but ferro-network was built without the `insecure-tls` \
         feature. Re-run with `cargo test --features insecure-tls` or ./scripts/live-test."
    );
    #[cfg(feature = "insecure-tls")]
    let builder = if insecure {
        builder.tls(ferro_network::TlsMode::AcceptInvalid)
    } else {
        builder
    };
    Some(builder.build().expect("live client builds"))
}

/// `true` only when `UNIFI_NETWORK_ALLOW_MUTATIONS` is truthy (`1`,
/// `true`, `yes`, `on`). Gates every `live_write_*` test.
#[expect(dead_code, reason = "the first live_write_* test lands in phase 6")]
pub fn mutations_allowed() -> bool {
    parse_boolish_env(ALLOW_MUTATIONS_ENV)
}

/// Boolish parsing matching the CLI's `--insecure` flag (clap's
/// `BoolishValueParser`): empty / `0` / `false` / `no` / `off` → false;
/// `1` / `true` / `yes` / `on` → true; unset → false; anything else
/// panics so a typo is loud.
fn parse_boolish_env(name: &str) -> bool {
    let Ok(raw) = std::env::var(name) else {
        return false;
    };
    match raw.trim().to_ascii_lowercase().as_str() {
        "" | "0" | "false" | "no" | "off" => false,
        "1" | "true" | "yes" | "on" => true,
        other => panic!(
            "{name} has unrecognised value {other:?}; use one of 1/true/yes/on or 0/false/no/off"
        ),
    }
}
