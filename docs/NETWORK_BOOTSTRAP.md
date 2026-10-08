# Bootstrap brief: `ferro-network` — a Rust client + CLI for the UniFi Network API

> **Historical — bootstrapping only.** This is the one-time brief that
> was handed to the coding agent to bootstrap this repository from an
> empty `git init`. It produced [PLAN.md](../PLAN.md),
> [AGENT.md](../AGENT.md), and [ARCHITECTURE.md](../ARCHITECTURE.md),
> which are now the authoritative documents; where they disagree with
> this brief, they win. Kept for context on why the project is shaped
> the way it is. Do not follow it as live instructions.

> **Audience**: a coding agent (Claude Code or otherwise) starting in a
> freshly `git init`-ed, empty repository. You have no prior context.
> Everything you need is in this file plus the reference repository it
> points at.

---

## 0. Your first job is a plan, not code

**Do not write any code, manifests, or scripts until the user has
approved a plan.** Your first deliverable is three documents, committed
together as `docs: initial plan, agent rules, architecture skeleton`:

1. **`PLAN.md`** — the phased build plan for this repo, modelled on the
   reference repo's `PLAN.md` (section 3 below tells you where to find
   it). Use the phase outline in section 6 of this brief as the
   starting point; refine it after reading the pinned Network spec
   yourself. Every phase gets: **Goal**, numbered **Tasks**, testing
   expectations, and a **Commit message**. Include a
   "Deferred — revisit before 0.1.0" section (empty is fine) and the
   "Reference: spec source" appendix.
2. **`AGENT.md`** — the operating rules in section 4 of this brief,
   adapted into a standalone file shaped like the reference repo's
   `AGENT.md`. Same rules; Network-specific names.
3. **`ARCHITECTURE.md`** — a *skeleton* only: the diagram, the "Why this
   shape" section, the key invariants, and an empty file map. It becomes
   a living document from phase 0 onward.

Then **stop**. Present a short summary of the plan to the user, list
every open question you could not answer from the spec (section 7 is a
starting list — resolve the ones you can by reading the spec, keep the
rest), and wait for approval. Revise the plan if asked. Only after an
explicit "go" do you start phase 0.

Do not commit until the user asks you to (see section 4, Commit policy).

---

## 1. What we are building

A Rust client library for the **UniFi Network local integration API**
(async, API-key auth) plus a CLI that exercises it. Two crates in one
Cargo workspace:

| Crate | Kind | Binary name |
|---|---|---|
| `ferro-network` | library — async client, typed models, errors | — |
| `ferro-network-cli` | `clap` binary — real tool *and* living integration test | `ferro-network` |

This is a sibling of **`ferro-protect`** (same author, same design),
which wraps the UniFi Protect API. We are deliberately reusing its
approach, design, rules, and phased delivery — including the lessons it
learned the hard way, which you should adopt from day one rather than
rediscover (section 5).

---

## 2. Repository state assumptions

- The user has already run `git init`. Do **not** run `git init`, rename
  `main`, or touch remote configuration.
- Add the spec as a submodule in phase 0 (not before):
  `git submodule add https://github.com/beezly/unifi-apis third_party/unifi-apis`.
  The Network specs live at `third_party/unifi-apis/unifi-network/{VERSION}.json`.
  Latest at the time of writing: **`11.0.81`** (OpenAPI 3.1.0, 73
  operations, 380 component schemas). Pin to that unless the user says
  otherwise; check `ls | sort -V` for newer.
- Repo slug: `lawliet89/ferro-network`
  (`repository = "https://github.com/lawliet89/ferro-network"` in
  `Cargo.toml`).

---

## 3. The reference implementation — read it before planning

Reference repo: **`lawliet89/ferro-protect`** on GitHub. When you need
to take a look, clone it into a temporary directory **outside** this
repo, read what you need, and delete the clone when you are done:

```sh
ref_dir="$(mktemp -d)"
gh repo clone lawliet89/ferro-protect "$ref_dir/ferro-protect" -- --depth 1
# ... read files under "$ref_dir/ferro-protect" ...
rm -rf "$ref_dir"
```

- Never clone it inside this working tree, add it as a submodule, or
  commit anything from it wholesale.
- Do not hardcode the clone location in any committed file; refer to
  the reference repo by its slug only.
- You can clone it again in later sessions or phases whenever you need
  to check how something was done. Clean up each time.
- The spec submodule is not needed to read the reference repo, so a
  shallow clone without `--recurse-submodules` is enough.

Read in this order:

1. `AGENT.md` — the rules (section 4 here is the adapted copy).
2. `ARCHITECTURE.md` — the shape you are reproducing.
3. `PLAN.md` — the phase structure you are mirroring. Note how each
   phase is library + CLI + tests, and how reads come before mutations.
4. `PROGRESS.md` — skim the headings, then read the entries for the
   oas3/typify migration, the rate limiter (both entries), edition 2024,
   CLI output, and config file. These are the decisions section 5 asks
   you to front-load.
5. `docs/TASK_*.md` — examples of self-contained chore briefs. Use the
   same format whenever a chore comes up mid-flight.
6. Source, in this order: `crates/ferro-protect/build.rs`,
   `build_support/spec_rewrite.rs`, `src/models.rs`, `src/client.rs`,
   `src/error.rs`, `src/rate_limit.rs`, `src/retry.rs`, `src/cameras.rs`
   (the per-entity wrapper template), then the CLI's `main.rs`,
   `config.rs`, `api_key.rs`, `output.rs`, `logging.rs`,
   `commands/cameras.rs`, and one test of each kind
   (`tests/cameras.rs` in both crates, `tests/live.rs`,
   `tests/common/mod.rs`).
7. Repo plumbing to copy-and-adapt: `Cargo.toml` (workspace deps + lint
   table), `rust-toolchain.toml`, `rustfmt.toml`, `deny.toml`,
   `.github/workflows/ci.yml`, `scripts/{pre-commit,update-spec,live-test}`,
   `.env.example`, `.gitignore`, `UPGRADING.md`, `LICENSE.md` (Apache-2.0).

**Copy shape, not text.** Where a file transfers almost verbatim
(lint table, `deny.toml`, CI), copy it and rename. Where it is
entity-specific, reproduce the pattern for Network's entities. Never
copy Protect-specific workarounds (e.g. `drop_drifted_audio_detection_enum`)
unless the Network spec demonstrably needs the same fix.

---

## 4. Operating rules (become `AGENT.md`)

These are the same rules as `ferro-protect`. Substitute names as
follows everywhere: `ferro-protect` → `ferro-network`,
`ProtectClient` → `NetworkClient`, `UNIFI_PROTECT_*` → `UNIFI_NETWORK_*`,
"NVR" → "UniFi console / Network application".

### 4.1 Orienting before changing anything

1. Read `PROGRESS.md` bottom-up until you know what was last in flight.
2. `git status` and `git log -10 --oneline`.
3. Read `ARCHITECTURE.md` if you have not this session.
4. Only then edit.

### 4.2 Commit policy

- One logical unit, one commit: one phase (or phase slice, where the
  plan says so) or one chore per commit.
- Conventional Commits: `phase(N): …`, `chore(<scope>): …`,
  `fix(<scope>): …`, `docs(<scope>): …`. Body explains *why* and
  cross-references the phase or task doc.
- Message via HEREDOC; `Co-Authored-By: Claude <noreply@anthropic.com>`
  trailer at the end (use the attribution line the harness provides, if
  it provides one).
- **Only commit when explicitly asked. Never push unless asked. Never
  force-push.**

### 4.3 PGP signing — non-negotiable

- Never use `--no-gpg-sign`, `--no-verify`, a hardcoded `-S` key, or any
  flag that bypasses signing or hooks.
- If a commit fails for anything signing-related: stop, tell the user
  the GPG cache likely needs warming
  (`echo unlock | gpg --clearsign --local-user 77820C080DD7DFC5 > /dev/null`),
  list the staged files and exact commit message, and wait.
- Same for `git push` if push signing is configured.

### 4.4 GitHub etiquette

- Every comment posted via `gh` (PR comments, reviews, issue comments,
  review-thread replies) **starts** with
  `> _Posted by <model name and version> via gh CLI (authenticated as the repo owner)._`
  Not needed for commit messages or PR descriptions.
- Replying to an inline review comment and resolving its thread is one
  operation (GraphQL `resolveReviewThread`). Exception: if you are
  declining the suggestion, leave the thread open.

### 4.5 Gates — all four green before every commit

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo deny check
```

- `#![forbid(unsafe_code)]` at the top of every `lib.rs` and `main.rs`.
  No `unsafe`, ever; if you think you need it, stop and log why.
- Unavoidable lint suppressions use `#[expect(..., reason = "...")]` on
  the smallest scope, logged in `PROGRESS.md`.

### 4.6 Progress logging

`PROGRESS.md` at the repo root, created on first run. Append one entry
per phase, chore, deviation, or surprise — one entry, one decision.

```markdown
## YYYY-MM-DD HH:MM ±HHMM — <phase or chore title>

**Status**: complete | partial | blocked

**Summary**:
<one paragraph>

**Files added/changed**:
- path/to/file

**Decisions / deviations**:
<anything off-plan, with reasoning>

**Next**: <what comes next, or what's blocking>
```

- Real timestamp from `date +"%Y-%m-%d %H:%M %z"` — the offset is part
  of the format.
- The entry for phase N is committed at the **start** of phase N+1
  (keeps phase commits topical). The final phase's entry gets its own
  follow-up commit. Chores commit their entry together with the chore.

### 4.7 Invariants

1. **Single source of truth for the spec version**:
   `crates/ferro-network/build.rs::SPEC_VERSION`. The path is derived
   from it. Bump only via `scripts/update-spec`.
2. **Hand-written code never names `crate::generated::…`.** Every type
   crossing a public signature is re-exported (optionally renamed) in
   `src/models.rs` — the seam that absorbs spec renames.
3. **Wrappers are mechanical.** Each entity method is a one-liner over
   shared HTTP helpers on `NetworkClient` (`get_json`, `post_json`,
   `put_json`, `patch_json`, `delete`, plus the paginated-list helper)
   and returns a `models::*` type. No inline URL or body construction in
   business logic.
4. **Request bodies use generated types where the spec defines them.**
   Free-form schemas get a hand-written builder with
   `#[serde(skip_serializing_if = "Option::is_none")]` and a comment
   pointing at the spec path.
5. **API keys live in `SecretString` end to end** — flag → builder →
   `HeaderValue` with `set_sensitive(true)`. Never `String`.
6. **`UNIFI_NETWORK_*` env vars are forbidden in CI.** The workflow
   fails fast if any are present.

If a task tempts you to break one, stop, reshape it, or log the reason
in `PROGRESS.md` and surface it to the user before proceeding.

### 4.8 Testing strategy — every endpoint ships three tests

1. **Mocked library test** — `crates/ferro-network/tests/<entity>.rs`,
   `wiremock`, committed JSON fixture under `tests/fixtures/`, happy
   path plus the most relevant error (401 for reads, 404 for `get`).
2. **CLI end-to-end test** — `crates/ferro-network-cli/tests/<entity>.rs`,
   `assert_cmd` against wiremock, asserting exit code, human stdout, and
   `--json` stdout. Wrap the sync `Command` in
   `tokio::task::spawn_blocking`.
3. **Live test** — `crates/ferro-network/tests/live.rs`. **Not**
   `#[ignore]`d; auto-skips when env is absent.

Live env contract (single `.env.local` drives CLI and tests):

- `UNIFI_NETWORK_HOST` — hostname or `host:port`, no scheme. Required;
  absent ⇒ all live tests skip.
- `UNIFI_NETWORK_API_KEY_FILE` or `UNIFI_NETWORK_API_KEY` — one required
  when `HOST` is set. `HOST` set with no key ⇒ helper **panics** with a
  clear message.
- `UNIFI_NETWORK_INSECURE` — truthy ⇒ accept self-signed TLS.
- `UNIFI_NETWORK_SITE` — site to run site-scoped live tests against
  (Network-specific; see section 5). Default: the site whose
  `internalReference` is `default`, else the first site.
- `UNIFI_NETWORK_ALLOW_MUTATIONS=1` — permits `live_write_*` tests. Test
  gate only; the CLI never reads it.

Naming: `live_read_*` (non-mutating) and `live_write_*` (mutating; also
skips unless mutations are allowed). Helpers in `tests/common/mod.rs`:
`live_client() -> Option<NetworkClient>`, `live_site_id()`,
`mutations_allowed() -> bool`. Prelude:

```rust
let Some(client) = common::live_client() else { return };
if !common::mutations_allowed() { return; } // live_write_* only
```

`scripts/live-test` sources `.env.local` and runs the live tests with
`--features insecure-tls --nocapture`.

**Insta, narrow scope**: only for deterministic pure transforms — CLI
`--help` text, canonical error messages, codegen seam smoke tests. Never
for response bodies; mocked tests assert specific fields, live tests
assert structure.

### 4.9 Logging

- Library emits via the `log` facade only; never initialises a logger.
- CLI wires `env_logger` in `src/logging.rs`. Precedence:
  `--log-level` > `UNIFI_NETWORK_LOG` > `RUST_LOG` > `warn`. Output to
  **stderr** so stdout stays parseable.
- `debug!` at request entry, `info!` on success with cardinality
  ("listed 12 devices"), `warn!` on unexpected fallback paths.
- Never log API keys or full request/response bodies.

### 4.10 Architecture doc maintenance

`ARCHITECTURE.md` is living. Update it when a structural decision,
module category, or invariant changes — not for the eleventh wrapper.
Target ~350 lines. Update the file map in the same commit as any new
top-level module. The release phase re-verifies it against reality.

### 4.11 Working style

- Many small, well-tested changes. No half-phases: "library + CLI +
  tests" means all three.
- Unclear plan or spec contradicts plan ⇒ pick the most defensible
  reading, log it in `PROGRESS.md`, keep going. Do not block on small
  questions.
- Non-blocking review findings go into PLAN.md's "Deferred" section with
  a **trigger condition**, not into a TODO comment.
- **Prefer stdlib traits**: canonical string form ⇒ `Display`; canonical
  conversion ⇒ `From`; parse ⇒ `FromStr`. Bespoke `as_*`/`to_*` only
  when a type has several distinct forms.
- **clap docs**: summarise the shared "flag > env > config file >
  default" chain once in the `Cli` struct doc; per-flag docs only
  describe deviations.
- **Push back on low-utility, high-LOC features.** Before writing code,
  estimate utility, LOC (incl. tests, docs, new deps), and risk. If the
  trade-off is poor, say so concretely and offer an alternative. The
  human decides — but you surface it *before* implementing.

---

## 5. Decisions to adopt from day one

`ferro-protect` reached these via chores and rewrites. Bake them into
phases 0–3 instead of repeating the journey. Read the cited
`PROGRESS.md` entries in the reference repo for the reasoning.

| Area | Decision | Protect history |
|---|---|---|
| Edition / resolver | Rust **2024**, `resolver = "3"`, `rust-version` set; `#[expect]` over `#[allow]` (`allow_attributes{,_without_reason} = "warn"`) | Started on 2021, migrated later |
| Codegen | **typify, models only**, from `components.schemas`. Hand-written `reqwest` wrappers. **No progenitor.** Generated code `include!`d in a permissively-allowed private `generated` module; `build_support/spec_rewrite.rs` holds pure structural preprocessing shared with tests | Started on progenitor, migrated after the rewrite layer exploded |
| Logging | `log` facade + `env_logger` in the CLI (not `tracing`) | Early chore |
| TLS | `rustls`; opt-in insecure mode behind a cargo feature named **`insecure-tls`** | Renamed from `dangerous-tls` |
| HTTP stack | `reqwest-middleware` with two clients: reads always retry; writes retry only with `retry_on_mutations(true)` | Added mid-phase-4 |
| Rate limiting | `governor` GCRA limiter + a small custom **Retry-After-aware** retry middleware (not `reqwest-retry`, which ignores `Retry-After`). **Measure the Network API's actual limit headers in phase 2** before picking defaults | Hand-rolled limiter replaced by governor |
| CLI output | `output.rs` with `emit()` (human vs `--json`) and a `comfy-table`-backed `table()` helper; per-entity `render_one()` stays manual | Custom renderer replaced |
| CLI config | TOML config file (`etcetera` XDG path, `UNIFI_NETWORK_CONFIG_FILE` override) + interactive `config` subcommand; precedence flag > env > file > default | Late chore |
| API key | Three sources, no raw key on the command line: `--api-key-file` > `UNIFI_NETWORK_API_KEY_FILE` > `UNIFI_NETWORK_API_KEY` (+ config file). Warn on group/world-readable key files | Phase 3 |
| Base URL | `--host` builds the default base URL; `--base-url` (mutually exclusive) overrides it for self-hosted or non-standard deployments | Added later |
| Live tests | Auto-skip model and shared helpers from the first live test — no `#[ignore]` phase | Migrated in a chore |
| CI | `permissions: contents: read`, concurrency cancel, `--locked`, `INSTA_UPDATE=no`, env-var guard before any `cargo test` | Hardened over several PRs |

### Network-specific differences you must design for

These do **not** exist in Protect. Verify each against the pinned spec
while writing `PLAN.md`.

1. **Base URL**: `https://{host}/proxy/network/integration` on UniFi OS
   consoles; paths start at `/v1/...`; auth header `X-API-Key` (the spec
   declares no security scheme — verify live). Self-hosted Network
   applications may differ (port 8443, no `/proxy/network` prefix) —
   that is what `--base-url` is for.
2. **Site scoping**: almost every path is `/v1/sites/{siteId}/...`.
   Design a `client.site(site_id)` → `SiteApi<'a>` handle whose
   sub-handles (`devices()`, `clients()`, `networks()`, …) carry the
   site ID, so wrappers stay one-liners. CLI gets a global `--site`
   (flag > `UNIFI_NETWORK_SITE` > config file > the `default` site),
   accepting either a UUID or a site `internalReference`/name resolved
   via `GET /v1/sites?filter=…`. Non-site endpoints (`/v1/info`,
   `/v1/pending-devices`, `/v1/dpi/*`, `/v1/countries`) live directly on
   the client.
3. **Pagination**: list endpoints take `offset` (default 0), `limit`
   (default 25, max 200), and `filter`, and return
   `{ offset, limit, count, totalCount, data }`. Build this once, in
   the shared helpers: a generic `Page<T>`, a `ListParams` builder, and
   an auto-paginating `list_all()` (a `Stream` or collected `Vec` —
   decide and log). CLI list commands get `--limit`, `--offset`,
   `--filter`, and `--all`. Every page DTO is a separately named schema;
   map them all onto `Page<T>` in `models.rs` or deserialize into
   `Page<T>` directly — decide in phase 4 and log it.
4. **Filter expressions**: the `filter` query param uses a small
   expression language (`eq`, `ne`, `in`, `notIn`, … per property,
   documented in each operation's description). Phase 4 decides whether
   to pass it through as a raw string (recommended to start) or build a
   typed builder (likely a "push back" candidate).
5. **Schema names contain spaces** (`"Site overview page"`,
   `"Error Message"`, `"Application info"`). Check how typify names
   them and add a preprocessing rule if needed. Pick final public names
   in `models.rs`.
6. **Discriminators without `oneOf`**: ~77 `discriminator` objects, e.g.
   `"Device action request"` has `discriminator.mapping` but only a bare
   `action: string` property, plus ~190 `allOf` uses. typify may not
   produce a tagged enum from that. Phase 1 must survey this and decide:
   structural rewrite into `oneOf` (preferred if mechanical) vs
   hand-written request types in `models.rs`. Log the choice.
7. **Error body**: `Error Message` = `{ statusCode, statusName, code,
   message, timestamp, requestPath, requestId }`. Map into
   `Error::Api { status, code, message, request_id }` with a raw-body
   fallback. Surface `requestId` on 5xx.
8. **PUT is full replacement.** Most config entities use `PUT` (replace)
   rather than `PATCH`; only firewall policies have `PATCH`. The CLI's
   update commands must not silently drop fields — plan a
   read-modify-write flow (GET → apply flags/`--patch-json` → PUT) with
   `--dry-run` printing the final body. Design this once in the first
   mutation phase.
9. **No WebSockets / binary endpoints** in the spec at 11.0.81. Protect's
   phases 5 and 7 have no equivalent; do not invent them.
10. **Blast radius is higher.** Network mutations can cut off the
    machine running the tests (networks, firewall, ACLs, WiFi, device
    restarts, port power-cycles). Every `live_write_*` must restore
    prior state, target only resources the test itself created where
    possible, and the README must say plainly: never run mutating live
    tests against a production site.

---

## 6. Phase outline (starting point for `PLAN.md`)

Mirror the reference ordering: scaffold → codegen → one end-to-end
slice → cross-cutting client concerns → **all reads** → **mutations in
increasing blast radius** → release. Reads always come first. Commit
granularity: one commit per phase, except where a phase says "commit per
entity group".

### Phase 0 — Workspace skeleton
Workspace manifest (`members = ["crates/*"]`, shared deps, the lint
table from the reference), toolchain/rustfmt/deny configs, both crates
as stubs with `#![forbid(unsafe_code)]`, CI (with `UNIFI_NETWORK_*`
guard and `permissions: contents: read`), `scripts/pre-commit`,
`.gitignore`, `.env.example`, `LICENSE.md`, stub `README.md`
(clone with `--recurse-submodules`, reserved "Running tests" heading),
empty `CHANGELOG.md`, spec submodule pinned to a commit.
Commit: `phase(0): set up workspace skeleton, lints, CI, submodule`.

### Phase 1 — Codegen pipeline
`build.rs` with `SPEC_VERSION = "11.0.81"`, `build_support/spec_rewrite.rs`,
typify models-only into `$OUT_DIR/generated.rs`, private
`generated.rs` include, `tests/model_codegen.rs` smoke test,
`scripts/update-spec` (lists versions under `unifi-network/`, bumps,
rebuilds, runs gates), `UPGRADING.md` (<120 lines, ends with an agent
checklist). Includes the schema-name and discriminator surveys from
section 5 items 5–6.
Commit: `phase(1): wire up typify model codegen from submoduled spec`.

### Phase 2 — First end-to-end slice: `info`
`error.rs`, `auth.rs`, `client.rs` (`NetworkClient` + builder: `host` /
`base_url`, `api_key`, `TlsMode`, timeouts), `models.rs` seam,
`client.info()` → `ApplicationInfo`. CLI skeleton with global args and
an `info` subcommand. All three test kinds, with the auto-skip live
model, `tests/common/mod.rs`, `scripts/live-test`, and the README
"Running tests" section done here (the reference needed a separate
chore for this). Record the rate-limit headers the server returns in
PROGRESS.md.
Commit: `phase(2): implement info endpoint end-to-end (library + CLI)`.

### Phase 3 — CLI configuration: API key resolver + config file
Three-source `api_key.rs` with `ApiKeyError`, TOML `config.rs`,
`config` subcommand, `logging.rs` precedence, global `--site`
plumbing (resolution lands in phase 4). Dedicated tests for every
precedence edge.
Commit: `phase(3): API key resolver, config file, and global option resolution`.

### Phase 4 — Rate limiting, retries, pagination, and sites
Cross-cutting client machinery before the entity fan-out:
`rate_limit.rs` + `retry.rs` (defaults from phase 2's measurements),
`Page<T>` / `ListParams` / `list_all`, site handle and site-name
resolution, then the first paginated entity: `sites list`. CLI
`--limit/--offset/--filter/--all`. Mocked tests for pagination edge
cases (empty, exactly one page, `totalCount` larger than returned).
Commit: `phase(4): rate limiting, retries, pagination, and sites`.

### Phase 5 — Read endpoints across all entities
One vertical slice per row (library + CLI + mocked + `assert_cmd` +
`live_read_*`), **commit per entity group**. Live `get` tests take the
first item from the list and skip cleanly when it is empty.

1. Devices: `devices list|get|stats` (latest statistics),
   `pending-devices list`.
2. Clients: `clients list|get`.
3. Networks: `networks list|get|references`.
4. WiFi broadcasts: `wifi list|get`.
5. Firewall: `firewall zones list|get`, `firewall policies list|get`,
   `firewall policies ordering`.
6. ACL rules: `acl list|get|ordering`.
7. DNS policies: `dns-policies list|get`.
8. Traffic matching lists: `traffic-lists list|get`.
9. Hotspot vouchers: `vouchers list|get`.
10. Switching: `switch-stacks`, `mc-lag-domains`, `lags` (list + get).
11. Supporting resources: `wans`, `vpn tunnels`, `vpn servers`,
    `radius-profiles`, `device-tags`, `dpi categories`,
    `dpi applications`, `countries`.

Commits: `phase(5): add devices read endpoints`, etc.

### Phase 6 — Mutations: create / update / delete (config entities)
First mutation phase. Establish the read-modify-write PUT pattern, named
flags for common fields, `--patch-json` escape hatch, and `--dry-run`
(asserted in tests to send nothing). Writes do not retry by default.
Order by blast radius, one commit per entity:

1. Hotspot vouchers — create, delete one, bulk delete by filter.
2. Traffic matching lists — create/update/delete.
3. DNS policies — create/update/delete.
4. WiFi broadcasts — create/update/delete.
5. Firewall zones — create/update/delete.
6. Firewall policies — create/update (PUT) / patch (PATCH) / delete /
   ordering.
7. ACL rules — create/update/delete/ordering.
8. Networks — create/update/delete (highest risk: can sever
   connectivity; live test operates only on a network it creates).

`live_write_*` tests create → read back → update → read back → delete,
or round-trip a value and restore it.

### Phase 7 — Action endpoints
One commit for the low-risk group, one for the rest:

1. Client actions (`clients action <id> …`).
2. Device actions (`devices restart <id>`), port actions
   (`devices port-action <id> <portIdx> …`, e.g. PoE power cycle).
3. Device adoption (`devices adopt`) and removal (`devices remove`).

All live tests are `live_write_*`. The CLI should require `--yes` (or an
interactive confirmation when stdout is a TTY) for restart/remove/
power-cycle — decide in the plan and apply consistently.

### Phase 8 — Polish and release prep
Rustdoc audit with a quickstart doctest; CLI `--help` audit; insta
snapshots for `--help` and stable error messages; full README (install,
CLI + library quickstarts, API key security, troubleshooting, testing);
`CHANGELOG.md` 0.1.0; crate versions 0.1.0; final pedantic/nursery
sweep; `tests/public_api.rs` compile-time canary touching every
`models::*` re-export; dry-run `scripts/update-spec` to the previous
spec version on a throwaway branch and revert; `ARCHITECTURE.md` sweep;
tag `v0.1.0` (only when asked).
Commit: `phase(8): docs, polish, release 0.1.0`.

---

## 7. Open questions to settle while planning

Answer from the spec where possible; ask the user about the rest in
your plan summary.

1. Crate names (`ferro-network` / `ferro-network-cli`)?
2. Pin spec `11.0.81`, or a different version?
3. Which deployment does the user test against (UniFi OS console vs
   self-hosted Network application)? Confirms the default base URL.
4. Is there a non-production site available for `live_write_*` tests?
5. `list_all()` as a `Stream` or an eager `Vec`? (Recommend: `Stream` in
   the library, collected in the CLI.)
6. Raw filter strings or a typed filter builder? (Recommend: raw string
   for 0.1.0; log the typed builder as deferred.)
7. Confirmation UX for destructive actions: `--yes` flag, TTY prompt, or
   both?
8. Is the `RESTART`-only device action and the discriminator pattern
   handled by a spec rewrite, or by hand-written request enums?

---

## 8. Definition of done for your first session

- [ ] Cloned the reference repo to a temp dir, read it per section 3,
      and deleted the clone.
- [ ] Read the pinned Network spec end to end (paths, page DTOs,
      discriminators, error schema).
- [ ] Wrote `PLAN.md`, `AGENT.md`, `ARCHITECTURE.md` (skeleton).
- [ ] Presented a short plan summary plus open questions to the user.
- [ ] **Stopped and waited for approval.** No code yet.
