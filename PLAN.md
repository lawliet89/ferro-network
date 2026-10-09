# UniFi Network Rust Client — Build Plan

This document is the phased delivery plan. Follow it phase by phase. Do
not skip ahead.

> **Before working on any phase or chore, read [AGENT.md](AGENT.md).**
> It carries the cross-cutting operating rules (commit policy, signing,
> gates, invariants, testing model, logging conventions). PLAN.md only
> carries the *what-to-do-next*.

## What we're building

A Rust client library for the UniFi Network local integration API
(spec **11.0.81**, async, API-key auth) plus a CLI that exercises it.
Two crates in one Cargo workspace:

| Crate | Kind | Binary name |
|---|---|---|
| `ferro-network` | library — async client, typed models, errors | — |
| `ferro-network-cli` | `clap` binary — real tool *and* living integration test | `ferro-network` |

The OpenAPI 3.1 spec comes from <https://github.com/beezly/unifi-apis>,
consumed as a git submodule. The pinned version lives in
`crates/ferro-network/build.rs::SPEC_VERSION`.

This is a sibling of `lawliet89/ferro-protect` and reuses its design,
rules, and phase structure. The decisions Protect reached through later
chores (edition 2024, typify models-only, `governor` + Retry-After
retry, `comfy-table` output, TOML config, auto-skip live tests,
hardened CI) are adopted from day one here — see the per-phase tasks.

### Phase ordering

Scaffold → codegen → one end-to-end slice → CLI configuration →
cross-cutting client machinery → **all reads** → **mutations in
increasing blast radius** → actions → release. Reads always come first.

### Spec at a glance (11.0.81, surveyed 2026-10-08)

- 73 operations across 50 paths; 380 component schemas; OpenAPI 3.1.0.
- `servers: [{url: "/integration"}]`; no `securitySchemes`. The auth
  header (`X-API-Key`) and the console base URL
  (`https://{host}/proxy/network/integration`) are **not** in the spec
  and must be confirmed live in phase 2.
- Only 2xx responses are documented (65× `200`, 8× `201`). No response
  headers are documented, so rate-limit behaviour must be measured live.
- Global (non-site) endpoints: `GET /v1/info`, `GET /v1/sites`,
  `GET /v1/pending-devices`, `GET /v1/dpi/categories`,
  `GET /v1/dpi/applications`, `GET /v1/countries`. Everything else is
  under `/v1/sites/{siteId}/…`.
- 23 paginated list endpoints, all with the same envelope
  `{offset, limit, count, totalCount, data}`. Query params: `offset`
  (default 0, min 0), `limit` (default 25, min 0, **max 200**),
  `filter` (string). `GET /v1/sites/{siteId}/wans` takes no `filter`.
- `filter` is a small expression language documented in the spec's
  "Filtering" tag (`prop.eq(…)`, `and(…)`, `not(…)`, typed literals);
  each operation's description lists its filterable properties.
  `DELETE /v1/sites/{siteId}/hotspot/vouchers` also takes `filter`
  (bulk delete).
- Error body schema `Error Message`: `statusCode`, `statusName`,
  `code` (e.g. `api.authentication.missing-credentials`), `message`,
  `timestamp`, `requestPath`, `requestId` (UUID; meaningful on 500).
- Polymorphism: **77 schemas carry a `discriminator` with a `mapping`
  but there are zero `oneOf`/`anyOf` anywhere.** Variants are expressed
  as `allOf: [<base>, {extra props}]` (190 `allOf` uses, 291 mapping
  targets). 279 targets `allOf` their base directly; 12 are shared
  across several unions (the `… entity metadata` family, one firewall
  protocol variant) or standalone (`Teleport client (connection)
  details`). This affects **responses** too (e.g. `Network details`,
  `Client details`, `Adopted device details`), not just requests.
- 197 of 380 schema names contain spaces or punctuation (`"Error
  Message"`, `"Teleport client (connection) overview"`). One pair
  collides after identifier sanitisation: `IP Address selector` vs
  `IP address selector`. 168 names are `Integration…Dto`.
- No `const`, no `type: [T, "null"]` arrays, no `pattern`. 72
  `format: uuid` (typify ⇒ `uuid::Uuid`), 13 `format: date-time`.
- Updates are `PUT` (full replacement) for every config entity; the only
  `PATCH` is firewall policies, and its body only carries
  `loggingEnabled` / `reportFlowEnabled`.
- No WebSocket or binary endpoints.

### Decisions settled during planning (2026-10-08)

1. **Crate names**: `ferro-network` (library) and `ferro-network-cli`
   (binary `ferro-network`).
2. **Spec version**: pin `11.0.81` (latest available at survey time).
3. **Deployment**: UniFi OS console. `--host` builds
   `https://{host}/proxy/network/integration`; `--base-url` stays as the
   override for self-hosted or non-standard setups. CLI and test
   configuration follow ferro-protect exactly: `UNIFI_NETWORK_*` env
   vars, `.env.local` + `scripts/live-test` for live tests, and the
   TOML config file with flag > env > file > default for the CLI only
   (live tests stay env-driven).
4. **`list_all()`**: a `Stream` in the library, collected into a `Vec`
   in the CLI.
5. **Filters**: raw `filter` strings for 0.1.0. A typed builder is
   deferred (see "Deferred").
6. **Confirmation UX for destructive commands** (every delete, bulk
   voucher delete, device restart / port power-cycle / remove /
   adopt): both. With `--yes`, proceed. Without `--yes` and with
   stdin and stdout both a TTY, prompt `Proceed? [y/N]` naming the
   operation and target; anything but `y`/`yes` aborts with a non-zero
   exit. Without `--yes` and not on a TTY, fail with an error telling
   the user to pass `--yes`. `--dry-run` never prompts. The prompt
   reads one line from stdin (no new prompt dependency; `is-terminal`
   only). Implemented once in a shared CLI helper in phase 6 and
   asserted in `assert_cmd` tests (non-TTY without `--yes` ⇒ error and
   no request sent).
7. **MSRV**: start at `rust-version = "1.85"`, as in ferro-protect;
   raise only when a dependency forces it, with a PROGRESS.md entry.

### Open questions

1. **Live mutation targets** — *parked until phase 6.* Is a
   non-production site available for `live_write_*` tests, and which
   device / port may phase 7's restart, power-cycle, and remove tests
   touch (via `UNIFI_NETWORK_TEST_DEVICE` / `UNIFI_NETWORK_TEST_PORT`)?
   Ask the user before writing the first `live_write_*` test. Until
   answered, mutating live tests are written but only exercised
   against whatever the user explicitly provides.

---

## Project layout (target state)

```
.
├── Cargo.toml                          # workspace manifest + lint table
├── rust-toolchain.toml
├── rustfmt.toml
├── deny.toml
├── .gitmodules
├── .github/workflows/ci.yml            # includes UNIFI_NETWORK_* guard
├── .gitignore
├── .env.example                        # template for UNIFI_NETWORK_* vars
├── scripts/
│   ├── pre-commit                      # fmt + clippy
│   ├── update-spec                     # one-command spec bump (phase 1)
│   └── live-test                       # source .env.local + run live tests
├── third_party/unifi-apis/             # submodule
├── crates/
│   ├── ferro-network/
│   │   ├── build.rs                    # SPEC_VERSION + typify codegen
│   │   ├── build_support/spec_rewrite.rs
│   │   ├── src/
│   │   │   ├── lib.rs, client.rs, auth.rs, error.rs
│   │   │   ├── models.rs               # THE SEAM
│   │   │   ├── generated.rs            # include!($OUT_DIR/generated.rs)
│   │   │   ├── pagination.rs           # Page<T>, ListParams, list_all
│   │   │   ├── rate_limit.rs, retry.rs
│   │   │   ├── site.rs                 # SiteApi<'a>
│   │   │   └── <entity>.rs             # one file per entity group
│   │   └── tests/
│   │       ├── common/mod.rs, fixtures/, live.rs, model_codegen.rs
│   │       └── <entity>.rs
│   └── ferro-network-cli/
│       ├── src/
│       │   ├── main.rs, lib.rs, api_key.rs, config.rs, logging.rs
│       │   ├── output.rs               # emit(), table()
│       │   ├── site.rs                 # --site resolution
│       │   └── commands/<entity>.rs
│       └── tests/<entity>.rs, common/mod.rs
├── AGENT.md, ARCHITECTURE.md, PLAN.md, PROGRESS.md
├── UPGRADING.md, CHANGELOG.md, README.md, LICENSE.md
```

---

## Phase 0 — Workspace skeleton

**Goal**: green `cargo build`, `fmt --check`, `clippy`, `test`, and
`deny check` on an empty two-crate workspace, with CI and the spec
submodule in place.

Tasks:

1. `Cargo.toml` workspace: `resolver = "3"`, `members = ["crates/*"]`,
   `[workspace.package]` with `edition = "2024"`, `rust-version`
   (start at ferro-protect's `1.85`; raise only if a dependency forces
   it and log why), `license = "Apache-2.0"`,
   `repository = "https://github.com/lawliet89/ferro-network"`,
   `authors`. Copy the reference lint tables verbatim: clippy
   `pedantic`/`nursery` warn, `module_name_repetitions`,
   `must_use_candidate`, `doc_markdown` allowed,
   `allow_attributes{,_without_reason} = "warn"`; rust
   `unsafe_code = "forbid"`, `unsafe_op_in_unsafe_fn = "deny"`,
   `unused_lifetimes = "warn"`. Release profile and `insta`/`similar`
   `[profile.dev.package]` overrides as in the reference.
   `[workspace.dependencies]` lists only what phase 0 uses; later phases
   add theirs.
2. `rust-toolchain.toml` (stable + `rustfmt`, `clippy`), `rustfmt.toml`
   (`edition = "2024"`, `max_width = 100`).
3. `deny.toml` adapted from the reference (license allow-list,
   advisories, sources incl. the unifi-apis URL).
4. `git submodule add https://github.com/beezly/unifi-apis third_party/unifi-apis`,
   pinned to the commit containing `unifi-network/11.0.81.json`
   (`c56902a` at survey time, or newer if it still contains 11.0.81).
5. `crates/ferro-network/` (lib, `lints.workspace = true`,
   `#![forbid(unsafe_code)]`, crate doc) and `crates/ferro-network-cli/`
   (binary `ferro-network`, stub `main`, `#![forbid(unsafe_code)]`).
6. `.github/workflows/ci.yml` adapted from the reference:
   `permissions: contents: read`, concurrency cancel-in-progress,
   checkout with `submodules: recursive`, the `UNIFI_NETWORK_*` guard
   **before** any cargo step, `--locked` on clippy/test/deny,
   `INSTA_UPDATE: "no"`, `Swatinem/rust-cache`.
7. `scripts/pre-commit` (fmt + clippy), `.gitignore` (`target/`,
   `.env`, `.env.local`, editor junk), `.env.example` (all
   `UNIFI_NETWORK_*` vars from AGENT.md, commented), `LICENSE.md`
   (Apache-2.0), stub `README.md` (one paragraph, clone with
   `--recurse-submodules`, reserved "Running tests" heading, a plain
   warning that mutating live tests must never target a production
   site), empty `CHANGELOG.md`.
8. Create `PROGRESS.md` with the phase 0 entry.
9. Run all four gates. Update the ARCHITECTURE.md file map.

Testing: gates only; no tests exist yet.

**Commit message**: `phase(0): set up workspace skeleton, lints, CI, submodule`

---

## Phase 1 — Codegen pipeline

**Goal**: `cargo build` generates Rust models from the 11.0.81 spec,
with real tagged enums for the discriminated unions, clippy clean on
hand-written code and silenced on generated code.

Tasks:

1. Dependencies: build-deps `typify` (latest), `schemars` (the version
   typify consumes), `serde_json`, `syn`, `prettyplease`; runtime deps
   needed by generated code (`serde`, `serde_json`, `uuid` with
   `serde`, `chrono` with `serde`, and `regress` only if typify needs
   it).
2. `build.rs` with `const SPEC_VERSION: &str = "11.0.81";` and the path
   derived from it. Mirror the reference: `rerun-if-changed` on spec,
   `build.rs`, and `build_support/spec_rewrite.rs`; friendly
   "run `git submodule update --init --recursive`" error; extract
   `components.schemas`; `TypeSpace::add_ref_types`; prettyplease to
   `$OUT_DIR/generated.rs`; derive `PartialEq`.
3. `src/generated.rs`: one reasoned `#![allow(...)]` block +
   `include!`. Declared `mod generated;` (private).
4. **Discriminator survey and decision (spec item 6).** Write down, in
   PROGRESS.md, what typify produces from the raw spec for three
   representative unions: `Device action request` (request, single
   variant, no extra fields), `Network details` (response, variants add
   fields), and `User or system defined entity metadata` (shared
   variants). Then implement the preferred fix, a structural rewrite
   `lift_discriminators_to_one_of` in `spec_rewrite.rs`:
   - for each schema `B` with `discriminator.mapping`, move B's own
     properties into a synthetic `B` "fields" schema (stable name,
     e.g. `"<B> base"`);
   - repoint every mapping target's `allOf` reference from `B` to the
     fields schema (breaks the cycle);
   - replace `B` with `oneOf` over the mapping targets, each pinned to
     its tag with a single-value `enum` on the discriminator property,
     so typify emits an internally tagged `#[serde(tag = "…")]` enum.
   - Targets that do not `allOf` their base (the 12 listed above) are
     handled by the same rule (the pin is added regardless); verify
     each compiles and round-trips.
   If typify cannot produce tagged enums from this shape after a
   time-boxed attempt, fall back to: hand-written request enums in
   `models.rs` for the action requests (tiny: `RESTART`,
   `POWER_CYCLE`, guest authorize/unauthorize), and `serde_json::Value`
   + `#[serde(flatten)]` capture for polymorphic responses. Log the
   choice either way.
5. **Schema-name survey (spec item 5).** Record how typify names
   space-separated schemas. Add a structural rule
   `disambiguate_case_colliding_names` that renames any schemas whose
   sanitised identifiers collide (today: `IP Address selector` /
   `IP address selector`) and rewrites `$ref`s accordingly. Final public
   names are chosen in `models.rs`, not in the rewrite.
6. Any further rules typify needs (e.g. `additionalProperties`
   alongside combinators, singleton `allOf` flattening) are ported from
   the reference **only when the Network spec triggers them**. Each rule
   gets a doc comment saying which schemas need it.
7. `tests/model_codegen.rs`: smoke test touching a handful of
   re-exports and round-tripping JSON for one tagged enum in each
   direction (request serialise, response deserialise) and for a page
   DTO. Unit tests for each `spec_rewrite` rule in the same file
   (the rewrite module is `#[path]`-included).
8. `scripts/update-spec`: list versions under
   `third_party/unifi-apis/unifi-network/` (`sort -V`) when called
   without args; otherwise fetch the submodule, verify the file exists,
   rewrite `SPEC_VERSION`, build, run all four gates, and print the new
   submodule SHA plus next-step git commands. Safe to re-run.
9. `UPGRADING.md` (< 120 lines): orientation, happy path, codegen
   failure triage (rewrite rules first, hand-written fallback second),
   wrapper compile failures (`models.rs` first), reading the generated
   diff under `target/debug/build/ferro-network-*/out/generated.rs`,
   and a numbered agent checklist.

Testing: `model_codegen.rs` + rewrite unit tests; `./scripts/update-spec`
with no args prints the version list.

**Commit message**: `phase(1): wire up typify model codegen from submoduled spec`

---

## Phase 2 — First end-to-end slice: `info`

**Goal**: `ferro-network info` against a real console prints the Network
application version. Library + CLI + all three test kinds, plus the
shared live-test infrastructure.

Tasks:

1. `error.rs`: `Error` with `Http`, `Middleware` (unused until phase 4
   but kept for shape), `Api { status: u16, code: String, message:
   String, request_id: Option<String> }`, `Json`, `InvalidUrl`,
   `MissingApiKey`, `Other`. `from_response` parses `Error Message`
   (`code`, `message`, `requestId`; fall back to `statusName` when
   `code` is empty), falls back to a truncated raw body with a `warn!`.
   `Display` includes the request ID when present on 5xx.
2. `auth.rs`: `ApiKey(SecretString)`, `API_KEY_HEADER = "X-API-Key"`,
   sensitive `HeaderValue`.
3. `client.rs`: `NetworkClient` + `NetworkClientBuilder` with `host`
   (builds `https://{host}/proxy/network/integration`), `base_url`
   (mutually exclusive override), `api_key`, `TlsMode { Native,
   Pinned(Vec<u8>), #[cfg(feature = "insecure-tls")] AcceptInvalid }`,
   connect/total timeouts (10s/30s). `get_json` helper. Plain
   `reqwest` for now; middleware lands in phase 4.
4. `models.rs` seam: `ApplicationInfo` re-exported from the generated
   `Application info` schema (it is named in this spec, unlike Protect).
5. `client.info()` → `GET /v1/info`.
6. Tests: `tests/info.rs` (wiremock happy path + 401 with an
   `Error Message` fixture asserting `code`), `tests/common/mod.rs`
   (`live_client()`, `mutations_allowed()`; `live_site_id()` lands in
   phase 4), `tests/live.rs::live_read_info`.
7. CLI: `Cli` with global `--host`, `--base-url`, `--insecure`,
   `--json`, `--log-level`, plus a **temporary** `--api-key-file` read
   directly (no env, no raw key flag) — replaced by the resolver in
   phase 3. `logging.rs` with flag > `UNIFI_NETWORK_LOG` > `RUST_LOG` >
   `warn`, stderr. `output.rs` with `emit()` and a `comfy-table`-backed
   `table()`. `info` subcommand: human prints the version; `--json`
   prints the object.
8. CLI test `tests/info.rs` (`assert_cmd` + wiremock, human + `--json`,
   `spawn_blocking`).
9. `scripts/live-test`, README "Running tests" section (quick start,
   live tests via `.env.local`, mutating live tests with the
   production-site warning, key-file `chmod 600`, CI guard).
10. **Live measurements, recorded in PROGRESS.md**: confirm base URL and
    `X-API-Key` on the user's deployment; confirm the self-hosted path
    shape if relevant; capture the rate-limit headers (`RateLimit-*`,
    `Retry-After`, anything else) from a single request and from a
    burst of ~20 parallel `GET /v1/info` calls; record whether a 429
    is ever returned and with what body.

Testing: all three kinds as above.

**Commit message**: `phase(2): implement info endpoint end-to-end (library + CLI)`

---

## Phase 3 — CLI configuration: API key resolver + config file

**Goal**: the CLI resolves every global option through one documented
precedence chain, never accepts a raw key on the command line, and can
persist defaults in a TOML file.

Tasks:

1. `api_key.rs`: `--api-key-file` > `UNIFI_NETWORK_API_KEY_FILE` >
   `UNIFI_NETWORK_API_KEY` > config `api_key_file`. Trims trailing
   whitespace, rejects empty files, warns (to an injected `io::Write`)
   when a key file is group/world readable. `ApiKeyError { NotProvided,
   ReadFailed { path, source }, EmptyFile(path) }`; `NotProvided`'s
   `Display` lists every source. No clap `env =` on the flag.
2. `config.rs`: TOML at the `etcetera` XDG path
   (`$XDG_CONFIG_HOME/ferro-network/config.toml`), override via
   `--config` > `UNIFI_NETWORK_CONFIG_FILE` (authoritative, missing ⇒
   error) > XDG default (opportunistic). Field precedence flag > env >
   file > default. Inline `api_key` in TOML is rejected with a
   sanitised error. `host`/`base_url` mutual exclusion checked across
   all sources in one place. `--insecure`/`--json` are `Option<bool>`
   with `require_equals` so `--json=false` can override a file value.
3. `config` subcommand: `show` (merged non-secret values), `path`,
   `template` (commented scaffold; `--stdout`).
4. Global `--site` plumbing: flag > `UNIFI_NETWORK_SITE` > config
   `site` > unset (resolved to the `default` site in phase 4). Stored
   as a raw string here; resolution lands in phase 4.
5. Remove the phase 2 temporary key handling; `info` uses the resolver.
6. Tests: `tests/api_key.rs` (every precedence edge, empty/missing/
   trimmed file, permission warning), `tests/cli_config.rs` (each field
   across flag/env/file, `host` vs `base_url` conflicts, inline-key
   rejection, `config show/path/template`), `tests/logging.rs`. CLI
   tests scrub `UNIFI_NETWORK_*` and XDG env vars via a shared helper
   in `tests/common/mod.rs`.

**Commit message**: `phase(3): API key resolver, config file, and global option resolution`

---

## Phase 4 — Rate limiting, retries, pagination, and sites

**Goal**: all cross-cutting client machinery exists before the entity
fan-out, proven on the first paginated entity: sites.

Tasks:

1. `rate_limit.rs`: `RateLimitConfig { rate: NonZeroU32, per: Duration }`
   + `governor` GCRA middleware. Defaults come from phase 2's
   measurements; if the server advertises nothing, pick a conservative
   default (e.g. 10/s), state it in the rustdoc, and log the reasoning.
2. `retry.rs`: `RetryAfterAwareMiddleware` (429, 5xx, 408, connect/read
   timeouts; honours `Retry-After`; exponential backoff + jitter;
   `RetryConfig` defaults 3 / 200ms / 5s). Two inner clients:
   `http_retriable` (GETs) and `http_mutating` (no retry unless
   `retry_on_mutations(true)`); one shared limiter.
   `From<reqwest_middleware::Error> for Error`.
3. `pagination.rs`:
   - `Page<T> { offset, limit, count, total_count, data: Vec<T> }`,
     deserialised directly from the wire envelope. **Decision**: the
     23 generated `…page` DTOs are not re-exported; wrappers return
     `Page<models::Foo>`, which keeps one pagination type in the public
     API. Log it.
   - `ListParams { offset, limit, filter }` builder; `limit` validated
     to `1..=200` client-side.
   - `get_page::<T>(path, &ListParams)` shared helper and
     `list_all::<T>(path, filter) -> impl Stream<Item = Result<T>>`
     that pages with `limit = 200` until `offset + count >= totalCount`
     or an empty page (guards against servers whose `totalCount`
     disagrees with what they return). CLI collects the stream.
4. `site.rs`: `client.site(id) -> SiteApi<'a>`; `client.sites()` with
   `list(&ListParams)` / `list_all(filter)`. `SiteId` is the public
   name for the UUID.
5. CLI: `site.rs` resolves `--site` (UUID ⇒ as-is; otherwise
   `GET /v1/sites?filter=or(internalReference.eq('x'),name.eq('x'))`;
   unset ⇒ `internalReference.eq('default')`, else the only site, else
   an error listing candidates). Resolution only runs for site-scoped
   commands. `sites list` with `--limit`, `--offset`, `--filter`,
   `--all` (shared `ListArgs` flattened into every list command).
   Human output shows `count of totalCount` when not `--all`.
6. Tests: `tests/rate_limit.rs` (Retry-After honoured, budget
   exhaustion, burst capped, writes not retried by default),
   `tests/pagination.rs` (empty, exactly one page, multi-page,
   `totalCount` larger than what the server ever returns, limit
   validation), `tests/sites.rs` (library + CLI incl. `--site`
   resolution by name / internalReference / UUID / ambiguity),
   `live_read_sites_list`, and `common::live_site_id()`.

**Commit message**: `phase(4): rate limiting, retries, pagination, and sites`

---

## Phase 5 — Read endpoints across all entities

**Goal**: a complete read-only inventory of a site via the CLI. Every
list and get the spec exposes.

One vertical slice per row (library + CLI + mocked + `assert_cmd` +
`live_read_*`), **commit per entity group**. Live `get` tests take the
first item from the list and skip cleanly when the list is empty. List
commands get `ListArgs`; human output is a `table()`, `get` uses a manual
`render_one()`; `--json` prints the object (or, for lists, the `data`
array — or the full page with `--json` + no `--all`; decide on the first
group and apply consistently).

1. **Devices**: `devices list|get|stats` (`GET …/devices`,
   `…/devices/{id}`, `…/devices/{id}/statistics/latest`) and
   `pending-devices list` (global).
2. **Clients**: `clients list|get`.
3. **Networks**: `networks list|get|references`.
4. **WiFi broadcasts**: `wifi list|get`.
5. **Firewall**: `firewall zones list|get`, `firewall policies
   list|get`, `firewall policies ordering --source-zone <id>
   --destination-zone <id>`.
6. **ACL rules**: `acl list|get|ordering`.
7. **DNS policies**: `dns-policies list|get`.
8. **Traffic matching lists**: `traffic-lists list|get`.
9. **Hotspot vouchers**: `vouchers list|get`.
10. **Switching**: `switch-stacks list|get`, `mc-lag-domains list|get`,
    `lags list|get`.
11. **Supporting resources** (list only — the spec has no `get`):
    `wans list` (no `--filter`), `vpn tunnels list`, `vpn servers
    list`, `radius-profiles list`, `device-tags list`, and the global
    `dpi categories list`, `dpi applications list`, `countries list`.

Polymorphic responses (`Network details`, `Client details`, …) render
their variant tag in human output.

**Commit messages**: `phase(5): add devices read endpoints`,
`phase(5): add clients read endpoints`, etc.

---

## Phase 6 — Mutations: create / update / delete (config entities)

**Goal**: CRUD for every config entity, in increasing blast radius.
First mutation phase; establishes the write patterns once.

Patterns, designed on the first entity and reused:

- **Library**: `create(&CreateBody)`, `update(id, &UpdateBody)` (PUT),
  `delete(id)` (+ `force` where the spec has it). Bodies are the
  generated `Create or update …` types re-exported in `models.rs`.
- **Read-modify-write update** in the CLI: GET the entity → serialise to
  JSON → drop read-only fields (`id`, `metadata`, …; the list is derived
  from the difference between the details and create/update schemas,
  documented per entity) → apply named flags, then `--patch-json`
  (RFC 7386 merge patch) → deserialise into the update type (validates
  the result) → PUT. Fields the user did not touch are preserved.
- **`--dry-run`** prints the final body and sends nothing (asserted in
  tests: wiremock expects zero PUT/POST/DELETE).
- **Create** takes named flags for the common fields and `--from-json
  <file|->` for the full body.
- **Delete** requires confirmation: `--yes`, or an interactive prompt
  on a TTY (decision 6 above). The shared confirmation helper lands
  with the first entity.
- Writes go through `http_mutating` and are not retried by default.

Order, one commit per entity:

1. Hotspot vouchers — create, delete one, bulk delete by filter. The
   bulk delete **requires** a non-empty `--filter` in the CLI and in the
   library signature (an empty filter on this endpoint may delete every
   voucher; verify live against a throwaway voucher only).
2. Traffic matching lists — create / update / delete.
3. DNS policies — create / update / delete.
4. WiFi broadcasts — create / update / delete (`--force`).
5. Firewall zones — create / update / delete (custom zones only).
6. Firewall policies — create / update (PUT) / patch (PATCH:
   `--logging`, `--report-flow`) / delete / reorder (PUT ordering).
7. ACL rules — create / update / delete / reorder.
8. Networks — create / update / delete (`--force`). Highest risk: the
   live test operates only on a network it creates (an unused VLAN ID)
   and never on the network the test host is on.

Before writing the first `live_write_*` test, resolve open question 1
with the user.

Live tests: `live_write_<entity>_crud` — create → read back → update →
read back → delete, with a cleanup guard that deletes on failure.

**Commit messages**: `phase(6): add hotspot voucher mutations`, etc.

---

## Phase 7 — Action endpoints

**Goal**: the "do a thing" POSTs and device lifecycle.

1. **Client actions** (one commit): `clients authorize-guest <id>
   [--time-limit-minutes …] [--data-limit-mb …] [--rx-kbps …]
   [--tx-kbps …]`, `clients unauthorize-guest <id>`.
2. **Device and port actions** (one commit with 3): `devices restart
   <id>`, `devices port power-cycle <id> <portIdx>`.
3. **Adoption and removal**: `devices adopt <mac> [--ignore-device-limit]`
   (`POST …/devices`), `devices remove <id>` (`DELETE …/devices/{id}`).

All require confirmation (`--yes` or a TTY prompt; decision 6 above).
Guest authorize/unauthorize is the exception: it is low-risk and
reversible, so it does not prompt. Request bodies come from
the phase 1 tagged enums (or the hand-written fallback). Live tests are
all `live_write_*`; restart / power-cycle / remove tests additionally
require an explicit per-test env var naming the target device ID
(`UNIFI_NETWORK_TEST_DEVICE`, `UNIFI_NETWORK_TEST_PORT`) and skip
without it, so a mutations-enabled run never picks a device on its own.

**Commit messages**: `phase(7): add client actions`,
`phase(7): add device actions, adoption, and removal`

---

## Phase 8 — Polish and release prep

1. Rustdoc audit: crate-level quickstart doctest; every public item
   documented.
2. CLI `--help` audit: grouping, every flag has help text.
3. `insta` snapshots for root + every subcommand `--help`
   (`tests/help_snapshot.rs`) and stable error messages
   (`tests/error_messages_snapshot.rs`) if they have settled.
4. Full README: install (`cargo install --path crates/ferro-network-cli`),
   CLI and library quickstarts, `--site` and `--filter` examples, API key
   security, troubleshooting (self-signed TLS, `--base-url` for
   self-hosted, generating an API key), testing, and the production-site
   warning.
5. `CHANGELOG.md` 0.1.0; both crates at `0.1.0`.
6. Final pedantic/nursery sweep.
7. `tests/public_api.rs`: compile-time canary touching every
   `models::*` re-export.
8. Dry-run `scripts/update-spec` to the previous spec version (10.6.106
   at survey time) on a throwaway branch; record the result in
   PROGRESS.md and UPGRADING.md; revert.
9. ARCHITECTURE.md sweep against the on-disk layout.
10. Tag `v0.1.0` — only when asked.

**Commit message**: `phase(8): docs, polish, release 0.1.0`

---

## Deferred — revisit before 0.1.0 (or when the trigger fires)

Items surfaced during planning or review that are not blocking. Move
each into a phase when its trigger fires, or close it out in PROGRESS.md
with a "won't do" rationale.

### Typed filter builder

**Symptom.** `filter` is passed through as a raw string. Typos surface
as server-side 400s, not compile errors.

**Trigger.** A library consumer (not the CLI) asks for it, or filter
misuse shows up repeatedly in bug reports. Until then, ~300+ LOC of
builder + per-entity property enums for little gain over a string and
the spec's documented syntax.

### Seal the `models.rs` seam against typify newtype tunnelling

**Symptom.** Same as ferro-protect: typify string newtypes expose
`.0`, letting callers reach past the seam.

**Trigger.** First spec bump that renames one of them, or the first
external consumer. (Less pressing here: most IDs are `uuid::Uuid`.)

### Publishing to crates.io

**Symptom.** Both crates set `publish = false`. `build.rs` reads the
spec from `third_party/unifi-apis`, outside the `ferro-network` package
root, so a crates.io archive would not contain it and every registry
build would fail in the build script.

**Trigger.** A decision to publish (at the latest, phase 8 before
tagging 0.1.0). Options then: copy the pinned spec into the package at
`cargo package` time (an `include`d file kept in sync by
`scripts/update-spec`, with `build.rs` preferring the submodule when
present), or ship the generated `models` source. Either way, add a CI
step that builds the packaged crate (`cargo package` + build from the
`.crate`) so the failure mode stays covered.

### Diagnosable decode errors for discriminated unions

**Symptom.** Lifted unions are `#[serde(untagged)]` enums (see
PROGRESS.md, phase 1). When a response drifts from the spec (a missing
"required" field, say), serde reports only `data did not match any
variant of untagged enum NetworkDetails`, not which field failed.

**Trigger.** The first live decode failure on a union that takes more
than a quick look to diagnose. Options then: a generated
`Deserialize` that peeks at the tag and decodes the matching variant
(post-processing typify's output in `build.rs`), or a debug helper that
retries each variant and reports every error.

### Firewall "named protocol" unions are not lifted

**Symptom.** In the three `Firewall policy … named protocol` schemas
the discriminator mapping keys (`AX_25`, `ICMPV6`) contradict the tag
property's own `enum` (`ax.25`, `icmpv6`), so `lift_discriminators_to_one_of`
leaves them as plain structs. The ICMP / ICMPv6 variants' extra
`typenameFilter` field is unreachable through them.

**Trigger.** Phase 5 firewall policies read (or phase 6 writes):
capture a live policy that uses a named protocol, see which spelling
the wire uses, and teach the rule that spelling.

---

## Reference: spec source

- Repo: <https://github.com/beezly/unifi-apis>
- Path in submodule: `third_party/unifi-apis/unifi-network/{SPEC_VERSION}.json`
  (currently `11.0.81.json`; latest available at survey time).
- Format: OpenAPI 3.1.0; `components.schemas` consumed by typify after
  structural preprocessing (phase 1).
- Spec `servers` entry: `/integration`; paths begin with `/v1/...`.
- Base URL on UniFi OS consoles: `https://{host}/proxy/network/integration`
  (not in the spec; confirmed live in phase 2). Self-hosted Network
  applications may differ — use `--base-url`.
- Auth: `X-API-Key` request header (not declared in the spec; confirmed
  live in phase 2).
- Self-signed TLS is the norm on consoles — `--insecure` (cargo feature
  `insecure-tls`) or `TlsMode::Pinned`.

## Reference: progress log template

See [AGENT.md → Progress logging](AGENT.md#progress-logging).
