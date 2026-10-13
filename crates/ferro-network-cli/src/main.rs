#![forbid(unsafe_code)]

//! `ferro-network` -- command-line tool for the UniFi Network local
//! integration API. Doubles as a living integration test for the
//! `ferro-network` library.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use ferro_network::{NetworkClient, TlsMode};
use ferro_network_cli::logging::{self, LogLevel};
use ferro_network_cli::output;
use secrecy::SecretString;

/// Command-line interface for the UniFi Network integration API.
#[derive(Debug, Parser)]
#[command(name = "ferro-network", version, about, long_about = None)]
struct Cli {
    /// Console hostname (or host:port), without scheme or path. The
    /// client wraps it as `https://{host}/proxy/network/integration`.
    #[arg(long, global = true, conflicts_with = "base_url")]
    host: Option<String>,

    /// Full base URL of the integration API, instead of --host: for a
    /// self-hosted Network application, the Site Manager cloud connector,
    /// or a test server.
    #[arg(long, global = true)]
    base_url: Option<String>,

    /// Path to a file containing the API key.
    #[arg(long, global = true)]
    api_key_file: Option<PathBuf>,

    /// Skip TLS certificate verification. Use only for a console whose
    /// self-signed certificate you cannot pin.
    #[arg(long, global = true)]
    insecure: bool,

    /// Emit JSON instead of human-readable output.
    #[arg(long, global = true)]
    json: bool,

    /// Log level for diagnostics on stderr. Overrides `UNIFI_NETWORK_LOG`,
    /// then `RUST_LOG`; the default is `warn`.
    #[arg(long, value_enum, global = true)]
    log_level: Option<LogLevel>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show application info (the running Network application version).
    Info,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    logging::init(cli.log_level.map(Into::into));

    let key_path = cli
        .api_key_file
        .as_deref()
        .ok_or_else(|| anyhow!("--api-key-file is required"))?;
    let key = read_api_key(key_path)?;

    let mut builder = NetworkClient::builder().api_key(key);
    builder = match (cli.host, cli.base_url) {
        (Some(host), _) => builder.host(host),
        (None, Some(url)) => builder.base_url(url),
        (None, None) => bail!("one of --host or --base-url is required"),
    };
    if cli.insecure {
        builder = builder.tls(TlsMode::AcceptInvalid);
    }
    let client = builder.build().context("failed to construct client")?;

    match cli.command {
        Command::Info => {
            let info = client.info().await.context("info request failed")?;
            output::emit_stdout(&info, cli.json, || {
                format!(
                    "Network application version: {}\n",
                    info.application_version
                )
            })?;
        }
    }
    Ok(())
}

/// Read the API key from `path`, trimming surrounding whitespace.
///
/// Temporary: phase 3 replaces this with the full resolver (env vars,
/// config file, permission warning).
fn read_api_key(path: &Path) -> Result<SecretString> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read API key file {}", path.display()))?;
    let key = raw.trim();
    if key.is_empty() {
        bail!("API key file {} is empty", path.display());
    }
    Ok(SecretString::from(key.to_string()))
}
