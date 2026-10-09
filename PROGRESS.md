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

---

## 2026-10-09 14:01 +0800 — Phase 1: typify codegen pipeline

**Status**: complete

**Summary**:
`build.rs` pins `SPEC_VERSION = "11.0.81"`, reads the spec from the
submodule, runs `build_support/spec_rewrite.rs::rewrite`, and feeds
`components.schemas` to typify 0.8 (models only, `PartialEq` derived),
writing `$OUT_DIR/generated.rs` (~26k lines: 453 structs, 297 enums).
`src/generated.rs` includes it privately under one reasoned
`#![allow]`; `src/models.rs` re-exports the first seam types
(`ApplicationInfo`, `NetworkDetails` + variants, `ClientActionRequest` +
variants). `tests/model_codegen.rs` fingerprints the seam, round-trips a
request union and a response union, and unit-tests each rewrite rule
plus whole-spec idempotence; an in-crate test checks a page DTO.
`scripts/update-spec` and `UPGRADING.md` added. All four gates green.

**Files added/changed**:
- `Cargo.toml`, `Cargo.lock`, `crates/ferro-network/Cargo.toml`
- `crates/ferro-network/build.rs`, `crates/ferro-network/build_support/spec_rewrite.rs`
- `crates/ferro-network/src/{lib.rs,generated.rs,models.rs}`
- `crates/ferro-network/tests/model_codegen.rs`, `crates/ferro-network/tests/fixtures/*.json`
- `scripts/update-spec`, `UPGRADING.md`
- `ARCHITECTURE.md` (status, union rationale, file map), `AGENT.md` (UPGRADING link), `PLAN.md` (two deferred items)

**Discriminator survey (raw spec, before lifting)**:
typify ignores `discriminator` entirely. `Device action request`
became `struct { action: String }`; `Network details` a struct with only
the base fields (`management: String`), so the `GATEWAY` / `SWITCH`
fields were unreachable from anything typed `NetworkDetails`; each
mapping target became its own flattened struct (`UnmanagedNetworkDetails`
a newtype of the base). `User or system defined entity metadata` was
`struct { origin: String }`, and its shared target `User defined entity
metadata` (which `allOf`s seven bases) collapsed to a newtype of an
unrelated identical struct. After lifting: 70 untagged enums of named
variants, single-variant unions become transparent newtypes
(`DeviceActionRequest(DeviceRestartRequest)`).

**Schema-name survey**: typify PascalCases names after replacing
non-identifier characters (`"Teleport client (connection) overview"` →
`TeleportClientConnectionOverview`; `Integration…Dto` names keep the
`Dto`). The only collision is `IP Address selector` / `IP address
selector` → `IpAddressSelector`; the rule renames the second to `IP
address selector 2` (the union, `IpAddressSelector2`). Public names are
chosen in `models.rs` when that type is first re-exported.

**Decisions / deviations**:
- **Rules the plan did not foresee**, each triggered by 11.0.81:
  `coerce_numeric_string_enums` (typify rejects the raw spec outright:
  `IntegrationWifiBasicDataRateConfigurationDto` declares `type:
  integer` with `enum: ["1000", ...]`) and
  `unconstrained_properties_to_true` (abstract bases declare
  `sourceFilter: {description}` / `access: {$ref: <annotation-only
  schema>}`; typify's `allOf` merge then inlined every narrowed union as
  anonymous `…Variant0` types — 198 of them — instead of using the named
  union). No Protect rules were ported: none is triggered.
- **Union representation** — see the next entry.
- **Unions skipped**: the three `Firewall policy … named protocol`
  schemas map `AX_25` / `ICMPV6` while their tag `enum` says `ax.25` /
  `icmpv6`. With the wire spelling unknown, the rule leaves any union
  whose tag `enum` contradicts its mapping as a plain struct. Deferred
  item added (trigger: phase 5 firewall reads).
- **Catch-all targets** (`…NamedProtocolDefaultDto`, mapped by up to 47
  tags) get one variant pinned to an `enum` of all their tags rather
  than one variant per tag.
- **typify 0.8.0** (latest stable; 0.10 is alpha) pulls syn 3 /
  prettyplease 0.3. `cargo deny` now warns about duplicate `syn` (2 via
  serde_derive / thiserror-impl, 3 via typify); build-time only.
- **Page DTOs are not re-exported** (phase 4 introduces `Page<T>`), so
  the plan's page round-trip lives as a unit test in `models.rs` rather
  than in `tests/model_codegen.rs`.
- **`update-spec` without args is read-only** (lists what the checked-out
  submodule has, no fetch), per the plan; the reference script fetched
  and moved the submodule even when only listing.
- Smoke-ran rewrite + typify against older specs: 10.6.106 (phase 8's
  dry-run target) and 9.0.99 pass; 10.0.162 hits another invalid
  `default`/`enum` construct. Not pursued: only 11.0.81 is pinned.

**Next**: Phase 2 — `info` end-to-end (library + CLI + live tests) and
live confirmation of base URL, auth header, and rate-limit headers.

---

## 2026-10-09 14:01 +0800 — Phase 1 deviation: unions are untagged enums of named variants

**Status**: complete

**Summary**:
PLAN.md phase 1 expected the lifted unions to become internally tagged
(`#[serde(tag = "…")]`) enums. typify 0.8 only emits internal tagging
when every `oneOf` variant is a literal inline object; it does not look
through `$ref`. Getting it would mean flattening every `allOf` chain
into inline variants, which turns `NetworkDetails::Gateway…(GatewayManagedNetworkDetails)`
into anonymous struct variants (no nameable payload type to re-export,
pass around, or build requests with) and duplicates shared variants
(`User defined entity metadata` appears in seven unions). Tried it: still
untagged plus `Variant0 { … }` inline structs.

Chosen instead: `oneOf` over `$ref`s to the named targets, each target
pinned to its tag. typify emits `#[serde(untagged)]` enums of newtype
variants. Decoding is unambiguous (exactly one variant's tag pin
matches), encoding writes the tag (it is a field of each payload, a
single-value enum), and unknown tags fail. Cost: serde's untagged error
on decode failure does not say which field failed — logged as a
deferred item with a trigger.

**Files added/changed**:
- `crates/ferro-network/build_support/spec_rewrite.rs`
- `ARCHITECTURE.md`, `PLAN.md` (deferred item)

**Decisions / deviations**:
The hand-written fallback (request enums in `models.rs`,
`serde_json::Value` for responses) was not needed.

**Next**: none; phase 2 builds on these types.
