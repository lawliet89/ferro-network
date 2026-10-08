# ferro-network — architecture

A start-here for developers (and agents) new to this codebase. The goal is
to load the shape of the project into your head before you start reading
source files.

If you want to **use** the library or CLI, read [README.md](README.md).
If you want to **bump the spec version**, read `UPGRADING.md`. The
**history of decisions** is in `PROGRESS.md`; the phased build plan is in
[PLAN.md](PLAN.md). If you are an **agent** about to make changes, read
[AGENT.md](AGENT.md) first.

This document is a living one. It is updated whenever a phase changes a
structural decision, adds a module category, or introduces an invariant.
See [AGENT.md → Architecture documentation maintenance](AGENT.md#architecture-documentation-maintenance).

> **Status: skeleton.** Written before phase 0. The diagram, rationale,
> and invariants describe the target shape; the file map is filled in as
> phases land.

---

## What this is

A Rust client for the UniFi Network local integration API, plus a CLI
that exercises it. Two crates in one Cargo workspace:

- **`ferro-network`** — async library. typify-generated models behind a
  hand-written `reqwest` client surface.
- **`ferro-network-cli`** — `clap`-based binary named `ferro-network`.
  Real tool; also a living integration test for the library.

The OpenAPI spec is consumed as a git submodule (`third_party/unifi-apis`)
— never vendored. One spec version is pinned at a time via a single
constant in `crates/ferro-network/build.rs`.

This is a sibling of `lawliet89/ferro-protect` and deliberately shares its
design. Where the two differ, it is because the Network API differs:
site scoping, pagination, discriminated unions, PUT-as-replace, and a
higher blast radius for mutations.

---

## The shape, in one diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                     beezly/unifi-apis (submodule)                   │
│              third_party/unifi-apis/unifi-network/*.json            │
└──────────────────────────────────┬──────────────────────────────────┘
                                   │ {SPEC_VERSION}.json
                                   ▼
┌─────────────────────────────────────────────────────────────────────┐
│  crates/ferro-network/build.rs                                      │
│  ─ reads the pinned spec                                            │
│  ─ delegates to build_support/spec_rewrite.rs::rewrite()            │
│    (structural only: discriminator → oneOf, name collisions, …)     │
│  ─ feeds components.schemas to typify (models only, no operations)  │
│  ─ writes $OUT_DIR/generated.rs                                     │
└──────────────────────────────────┬──────────────────────────────────┘
                                   ▼
┌─────────────────────────────────────────────────────────────────────┐
│  src/generated.rs  — private; include!(…/generated.rs);             │
│  permissively allowed so generated code never trips the lint gate   │
└──────────────────────────────────┬──────────────────────────────────┘
                                   │ generated model types
                                   ▼
┌─────────────────────────────────────────────────────────────────────┐
│  src/models.rs    ← THE SEAM                                        │
│  ─ pub use crate::generated::Foo (renamed to clean public names)    │
│  ─ Page<T>, ListParams, and any hand-written request/response types │
│  ─ the ONLY place hand-written code names generated types           │
└──────────────────────────────────┬──────────────────────────────────┘
                                   │ models::*
                                   ▼
┌─────────────────────────────────────────────────────────────────────┐
│  NetworkClient (src/client.rs, error.rs, auth.rs, pagination.rs)    │
│  ─ shared helpers: get_json / post_json / put_json / patch_json /   │
│    delete / get_page / list_all                                     │
│  ─ reqwest-middleware stack: rate limit → Retry-After-aware retry   │
│  ─ two inner clients: retriable (reads) and mutating (no retry)     │
│  ─ uniform Error mapping from the `Error Message` body              │
└───────────────┬─────────────────────────────────────┬───────────────┘
                │ global endpoints                    │ client.site(id)
                ▼                                     ▼
┌───────────────────────────────┐   ┌─────────────────────────────────┐
│ info(), pending_devices(),    │   │ SiteApi<'a> { client, site_id } │
│ dpi(), countries(), sites()   │   │ ─ devices(), clients(),         │
│                               │   │   networks(), wifi(),           │
│                               │   │   firewall(), acl_rules(), …    │
│                               │   │ each a one-line-wrapper handle  │
└───────────────┬───────────────┘   └────────────────┬────────────────┘
                └──────────────────┬─────────────────┘
                                   ▼ public API
            ┌──────────────────────┴───────────────────────┐
            ▼                                              ▼
┌─────────────────────────┐                  ┌──────────────────────────┐
│  ferro-network-cli      │                  │  external consumers      │
│  clap CLI, --json,      │                  │  (your code)             │
│  global --site          │                  │                          │
└─────────────────────────┘                  └──────────────────────────┘
```

---

## Why this shape

### Codegen for models, hand-written HTTP

The Network spec (11.0.81) defines 380 component schemas and 73
operations. Hand-writing the models would be rote, and every spec bump
would be a manual diff session. `typify` consumes `components.schemas`
at build time and emits the Rust model types. Operations are *not*
generated: `ferro-protect` started on progenitor and migrated away once
its rewrite layer exploded. A hand-written `reqwest` surface over shared
helpers is small, uniform, and easy to review.

### A structural rewrite layer, kept pure

`build_support/spec_rewrite.rs` applies schema-only preprocessing before
typify sees the spec. It is a pure function (`Value -> Value`, no I/O),
shared with tests, and every rule matches on JSON Schema *structure*,
never on runtime-observed values. The Network spec needs rules Protect
did not, most importantly turning the spec's
`discriminator`-without-`oneOf` pattern into real tagged unions (see
PLAN.md phase 1).

### The `models.rs` seam absorbs spec changes

Hand-written code **never** names `crate::generated::Foo`. Every type
crossing a public signature is re-exported, usually renamed, in
`src/models.rs`. Network schema names are prose (`"Adopted device
overview page"`, `"Error Message"`), so renaming here is the norm, not
the exception. When a spec bump renames a type, the fix lives in
`models.rs`.

### Site handles keep wrappers one-liners

Almost every path is `/v1/sites/{siteId}/...`. Rather than threading a
`site_id` argument through ~60 methods, `client.site(id)` returns a
cheap `SiteApi<'a>` borrowing the client and carrying the ID; its
sub-handles (`devices()`, `networks()`, …) build paths from it. The few
global endpoints (`/v1/info`, `/v1/sites`, `/v1/pending-devices`,
`/v1/dpi/*`, `/v1/countries`) hang directly off `NetworkClient`.

### Pagination is built once

All 23 list endpoints share one envelope (`offset`, `limit`, `count`,
`totalCount`, `data`) and the same `offset`/`limit`/`filter` query
params. The client owns a generic `Page<T>`, a `ListParams` builder, and
an auto-paginating `list_all()`. Wrappers never deal with offsets.

### Reads retry, writes do not

Two `reqwest-middleware` clients share one rate limiter. The retriable
client wraps a Retry-After-aware retry layer and serves GETs. The
mutating client skips retries unless
`NetworkClientBuilder::retry_on_mutations(true)` is set, so a 5xx after
the controller already applied a change is never silently re-sent.

### PUT replaces, so the CLI reads before it writes

Most config entities are updated with `PUT` (full replacement). The CLI's
update commands do GET → apply flags / `--patch-json` → PUT, and
`--dry-run` prints the final body without sending. Fields the user did
not mention are preserved, never silently dropped.

---

## Key invariants

These hold across every phase. See [AGENT.md](AGENT.md#invariants-you-must-preserve).

1. **`#![forbid(unsafe_code)]`** at the top of every `lib.rs` and
   `main.rs`.
2. **No `crate::generated::...` in public signatures.** `models.rs` is
   the only crossing point.
3. **One `SPEC_VERSION` constant** in `crates/ferro-network/build.rs`.
4. **Wrappers are mechanical** one-liners over the shared helpers.
5. **Generated code is permissively allowed; hand-written code is held
   to `pedantic + nursery` with `-D warnings`**, and suppressions use
   `#[expect(..., reason = "...")]`.
6. **Every commit passes all four gates** (fmt, clippy, test, deny).
7. **API keys live in `SecretString`** end to end.
8. **`UNIFI_NETWORK_*` env vars are forbidden in CI.**
9. **Mutating live tests restore state** and only target resources they
   created where possible.

---

## File map

The current state. Updated in the same commit as any new top-level
module.

### Repo root

| Path | What |
|---|---|
| [AGENT.md](AGENT.md) | Operating rules for coding agents. |
| [ARCHITECTURE.md](ARCHITECTURE.md) | This file. |
| [PLAN.md](PLAN.md) | Phased build plan, settled decisions, open questions, deferred items. |
| [PROGRESS.md](PROGRESS.md) | Chronological decision log. |
| [Cargo.toml](Cargo.toml) | Workspace manifest. `resolver = "3"`, edition 2024, shared `[workspace.dependencies]`, lint policy (`pedantic + nursery` warn; `unsafe_code = "forbid"`; `allow_attributes{,_without_reason}` push `#[allow]` → `#[expect(reason)]`). |
| [rust-toolchain.toml](rust-toolchain.toml) | Pins the stable channel + `rustfmt`, `clippy`. |
| [rustfmt.toml](rustfmt.toml) | `edition = "2024"`, `max_width = 100`. |
| [deny.toml](deny.toml) | License allow-list, advisories, source allow-list. |
| [.github/workflows/ci.yml](.github/workflows/ci.yml) | fmt → clippy → test → deny. Refuses to run if `UNIFI_NETWORK_*` env vars are present. |
| [scripts/pre-commit](scripts/pre-commit) | Local hook: fmt + clippy. |
| [.env.example](.env.example) | Template for `UNIFI_NETWORK_*` vars. |
| [docs/](docs/) | Chore briefs (`TASK_*.md`) and the historical bootstrap brief. |

### `crates/ferro-network/` (library)

| Path | What |
|---|---|
| [Cargo.toml](crates/ferro-network/Cargo.toml) | Library manifest. |
| [src/lib.rs](crates/ferro-network/src/lib.rs) | Crate root. Stub until phase 1. |

### `crates/ferro-network-cli/` (CLI)

| Path | What |
|---|---|
| [Cargo.toml](crates/ferro-network-cli/Cargo.toml) | CLI manifest; binary name `ferro-network`. |
| [src/main.rs](crates/ferro-network-cli/src/main.rs) | Stub until phase 2. |

### `third_party/unifi-apis/` (submodule)

The OpenAPI specs published at <https://github.com/beezly/unifi-apis>,
pinned at a specific commit. The current spec is
`third_party/unifi-apis/unifi-network/{SPEC_VERSION}.json`.

---

## Where to start reading

_Filled in once there is code to read (phase 2)._
