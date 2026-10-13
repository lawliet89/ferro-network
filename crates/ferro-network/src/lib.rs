#![forbid(unsafe_code)]

//! Async Rust client for the UniFi Network local integration API.
//!
//! Pre-0.1.0: only the generated models exist so far (see [`models`]).
//! See `PLAN.md` at the repository root for the phased build plan.

pub mod models;

mod generated;
