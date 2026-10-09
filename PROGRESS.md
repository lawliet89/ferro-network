# Progress log

Chronological record of phases, chores, deviations, and surprises. See
[AGENT.md → Progress logging](AGENT.md#progress-logging) for the entry
format and commit timing. Newest entries at the bottom.

---

## 2026-10-08 11:08 +0800 — Phase 0: workspace skeleton, lints, CI, submodule

**Status**: complete

**Summary**:
Created the two-crate Cargo workspace (`ferro-network` library,
`ferro-network-cli` with binary `ferro-network`), edition 2024 /
resolver 3 / `rust-version = "1.85"`, and the lint tables copied from
ferro-protect (pedantic + nursery warn, `allow_attributes{,_without_reason}`
warn, `unsafe_code` forbid). Added toolchain, rustfmt, and cargo-deny
configs, the CI workflow (contents-read permissions, concurrency cancel,
`UNIFI_NETWORK_*` guard before any cargo step, `--locked`,
`INSTA_UPDATE=no`), `scripts/pre-commit`, `.gitignore`, `.env.example`,
Apache-2.0 `LICENSE.md`, stub `README.md` and `CHANGELOG.md`, and the
`third_party/unifi-apis` submodule pinned at `c56902a` (contains
`unifi-network/11.0.81.json`, still the latest). All four gates green.

**Files added/changed**:
- `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `rustfmt.toml`, `deny.toml`
- `crates/ferro-network/{Cargo.toml,src/lib.rs}`
- `crates/ferro-network-cli/{Cargo.toml,src/main.rs}`
- `.github/workflows/ci.yml`, `scripts/pre-commit`
- `.gitignore`, `.env.example`, `.gitmodules`, `third_party/unifi-apis`
- `LICENSE.md`, `README.md`, `CHANGELOG.md`, `PROGRESS.md`
- `ARCHITECTURE.md` (file map)

**Decisions / deviations**:
- **`insta`/`similar` `[profile.dev.package]` overrides deferred** to the
  phase that first adds `insta`. With neither crate in the graph, cargo
  warns `profile package spec ... did not match any packages` on every
  invocation. Re-add them verbatim from ferro-protect then.
- **`deny.toml` gained `unused-allowed-license = "allow"`.** The licence
  allow-list is copied from ferro-protect and seeded for the reqwest /
  rustls / typify / clap stack later phases add; with zero dependencies
  every entry warned as unmatched. The `allow-git` entry for
  `beezly/unifi-apis` still emits a harmless "no crate source matched"
  warning (same as the reference: the submodule is not a cargo source).
- **CLI stub prints its version** (`ferro-network 0.1.0`) instead of an
  empty `fn main() {}`, which trips `clippy::missing_const_for_fn` under
  nursery. Replaced in phase 2.
- **`[workspace.dependencies]` starts empty**; each phase adds what it
  uses rather than pre-declaring the whole reference list.
- Moved the bootstrap brief to `docs/NETWORK_BOOTSTRAP.md` with a
  "historical, bootstrapping only" header; it is committed separately
  from phase 0 at the user's request.

**Next**: Phase 1 — typify codegen pipeline, discriminator and
schema-name surveys.
