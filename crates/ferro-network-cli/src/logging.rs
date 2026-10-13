//! Logger initialisation for the `ferro-network` binary.
//!
//! Filter resolution order, highest priority first:
//!
//! 1. The `--log-level` flag (passed to [`init`] by the caller).
//! 2. `UNIFI_NETWORK_LOG` (env_logger filter syntax, like `RUST_LOG`).
//! 3. `RUST_LOG`.
//! 4. The literal default `warn`.
//!
//! Logs go to **stderr**, so `--json` and human output on stdout stay
//! parseable.

use std::io::Write;

use clap::ValueEnum;

const APP_ENV: &str = "UNIFI_NETWORK_LOG";
const DEFAULT_FILTER: &str = "warn";

/// `--log-level` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<LogLevel> for log::LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Error => Self::Error,
            LogLevel::Warn => Self::Warn,
            LogLevel::Info => Self::Info,
            LogLevel::Debug => Self::Debug,
            LogLevel::Trace => Self::Trace,
        }
    }
}

/// Initialise the global logger. `cli_level` is the `--log-level` value.
///
/// Calling it twice is a no-op: env_logger's `try_init` error ("logger
/// already set") is ignored.
pub fn init(cli_level: Option<log::LevelFilter>) {
    let mut builder = env_logger::Builder::new();
    // A fixed format keeps test assertions stable; the module path is
    // added at debug and below for grep-ability.
    builder.format(|buf, record| {
        let ts = buf.timestamp();
        if record.level() <= log::Level::Info {
            writeln!(buf, "{ts} {} {}", record.level(), record.args())
        } else {
            writeln!(
                buf,
                "{ts} {} {} {}",
                record.level(),
                record.target(),
                record.args()
            )
        }
    });
    builder.target(env_logger::Target::Stderr);

    if let Some(level) = cli_level {
        builder.filter_level(level);
    } else if let Ok(filter) = std::env::var(APP_ENV) {
        builder.parse_filters(&filter);
    } else if let Ok(filter) = std::env::var("RUST_LOG") {
        builder.parse_filters(&filter);
    } else {
        builder.parse_filters(DEFAULT_FILTER);
    }

    let _ = builder.try_init();
}
