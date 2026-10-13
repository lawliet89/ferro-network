#![forbid(unsafe_code)]

//! Library half of the `ferro-network` CLI. Exists so CLI internals can
//! be unit-tested and reused by the binary in `main.rs`.

pub mod logging;
pub mod output;
