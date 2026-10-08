#![forbid(unsafe_code)]

//! `ferro-network` -- command-line tool for the UniFi Network local
//! integration API. Doubles as a living integration test for the
//! `ferro-network` library.
//!
//! Pre-0.1.0: a stub until phase 2 adds the first subcommand.

fn main() {
    println!("ferro-network {}", env!("CARGO_PKG_VERSION"));
}
