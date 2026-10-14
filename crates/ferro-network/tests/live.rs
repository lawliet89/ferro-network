#![forbid(unsafe_code)]

//! Live integration tests against a real Network application.
//!
//! They run as part of `cargo test --all` and return early (passing)
//! when neither `UNIFI_NETWORK_HOST` nor `UNIFI_NETWORK_BASE_URL` is set. Configure `.env.local` from
//! `.env.example`, then:
//!
//! ```sh
//! ./scripts/live-test                 # sources .env.local for you
//! ```
//!
//! `live_read_*` tests never change state. `live_write_*` tests (from
//! phase 6) additionally require `UNIFI_NETWORK_ALLOW_MUTATIONS=1` and
//! must never target a production site. See AGENT.md "Testing strategy".

mod common;

#[tokio::test]
async fn live_read_info() {
    let Some(client) = common::live_client() else {
        println!("(skipping live_read_info: no live target configured)");
        return;
    };
    let info = client.info().await.expect("info call succeeds");
    println!(
        "live_read_info: Network application version = {}",
        info.application_version
    );
    assert!(
        !info.application_version.is_empty(),
        "console returned an empty applicationVersion"
    );
}
