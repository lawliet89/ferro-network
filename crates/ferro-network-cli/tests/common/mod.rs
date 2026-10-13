//! Shared helpers for CLI integration tests.
//!
//! [`isolated_cmd`] builds a `Command` for the freshly built binary with
//! every env var the CLI reads scrubbed, so a developer's `.env.local`
//! or shell setup can never leak into a test (or send it to a real
//! console).

use std::io::Write;

use assert_cmd::Command;
use tempfile::NamedTempFile;

/// Every env var the CLI reads, or will read once phase 3 lands.
pub const SCRUBBED_ENV_VARS: &[&str] = &[
    "UNIFI_NETWORK_HOST",
    "UNIFI_NETWORK_BASE_URL",
    "UNIFI_NETWORK_API_KEY",
    "UNIFI_NETWORK_API_KEY_FILE",
    "UNIFI_NETWORK_INSECURE",
    "UNIFI_NETWORK_JSON",
    "UNIFI_NETWORK_SITE",
    "UNIFI_NETWORK_LOG",
    "UNIFI_NETWORK_CONFIG_FILE",
    "RUST_LOG",
    "XDG_CONFIG_HOME",
];

/// The `ferro-network` binary with a scrubbed environment.
pub fn isolated_cmd() -> Command {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ferro-network"));
    for var in SCRUBBED_ENV_VARS {
        cmd.env_remove(var);
    }
    cmd
}

/// A temp file holding `key` (with a trailing newline, as editors write
/// it). Keep the returned handle alive while the command runs.
pub fn key_file(key: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("temp file");
    writeln!(file, "{key}").expect("write key");
    file
}
