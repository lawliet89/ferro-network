#![forbid(unsafe_code)]

//! Async Rust client for the UniFi Network local integration API.
//!
//! ```no_run
//! # async fn run() -> ferro_network::Result<()> {
//! use ferro_network::NetworkClient;
//! use secrecy::SecretString;
//!
//! let client = NetworkClient::builder()
//!     .host("unifi.local")
//!     .api_key(SecretString::from("paste-key-here".to_string()))
//!     .build()?;
//!
//! let info = client.info().await?;
//! println!("Network {}", info.application_version);
//! # Ok(()) }
//! ```
//!
//! Pre-0.1.0: see `PLAN.md` at the repository root for the phased build
//! plan.
//!
//! # Logging
//!
//! This crate emits log records through the [`log`](https://docs.rs/log)
//! facade and never initialises a logger itself; the binary using it
//! chooses one (`env_logger`, `tracing-log`, ...). The `ferro-network`
//! CLI uses `env_logger`.
//!
//! Levels emitted:
//!
//! - `info!`  -- client built (with its TLS mode), request outcomes
//! - `debug!` -- each outbound request, builder timeouts
//! - `warn!`  -- fallback paths, such as an error body that is not the
//!   API's `Error Message` shape
//!
//! API keys and request/response bodies are never logged.

pub mod error;
pub mod models;

mod auth;
mod client;
mod generated;

pub use client::{NetworkClient, NetworkClientBuilder, TlsMode};
pub use error::{ApiError, Error, Result};
