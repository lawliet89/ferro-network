# AGENT.md — operating instructions for coding agents in this repo

If you are an agent (Claude Code or otherwise) working on this codebase,
read this file before touching anything else. It is the cross-cutting
contract: commit conventions, signing, gates, invariants, testing rules,
logging conventions, and how to leave a useful trail behind you. It is
deliberately short — the per-task detail lives elsewhere.

For *what* to work on:

- **Phased build plan** — [PLAN.md](PLAN.md). Phases 0-8 plus deferred
  items. Read only the phase or section relevant to your task.
- **One-off chores** — `docs/TASK_*.md` (none yet). Self-contained
  briefs in the shape used by the sibling repo `lawliet89/ferro-protect`:
  *Why*, *Timing*, *Scope decision*, numbered tasks, verification,
  commit message. Each task doc links its prerequisites; you do not
  need PLAN.md to do them. Write one whenever a chore comes up
  mid-flight rather than folding it silently into a phase.

For *how the code is shaped*:

- **Architecture overview** — [ARCHITECTURE.md](ARCHITECTURE.md).
  Diagram, file map, invariants, reading order.
- **Spec upgrade procedure** — `UPGRADING.md` (lands in phase 1).
- **Historical decisions** — `PROGRESS.md` (created on first run).
  Chronological log; read when something in the code surprises you.

This project deliberately mirrors `lawliet89/ferro-protect` (same
author, same design). When you need to see how something was done
there, clone it into a temporary directory **outside** this working
tree, read what you need, and delete the clone:

```sh
ref_dir="$(mktemp -d)"
gh repo clone lawliet89/ferro-protect "$ref_dir/ferro-protect" -- --depth 1
# ... read files under "$ref_dir/ferro-protect" ...
rm -rf "$ref_dir"
```

Never clone it inside this repo, add it as a submodule, or copy it
wholesale. Copy shape, not text. Never copy Protect-specific
workarounds unless the Network spec demonstrably needs the same fix.

---

## Orienting yourself before changing anything

1. Read PROGRESS.md from the bottom up until you understand what was
   most recently in flight. The phase/chore you've been asked to do may
   have prerequisites or recent deviations not yet reflected in PLAN.md.
2. `git status` and `git log -10 --oneline` to confirm the branch state
   matches what the user implied.
3. Read [ARCHITECTURE.md](ARCHITECTURE.md) if you have not already in
   this session — it is the fastest path from zero to "I know where
   things live."
4. Only then start editing.

## Repository state assumptions

The user has already run `git init`. Treat the working directory as
your repo root. Do **not** run `git init`, do **not** rename `main`, do
**not** rewrite remote configuration.

The submodule at `third_party/unifi-apis` may or may not be checked out
when you start. If you need it and it is empty, run:

```sh
git submodule update --init --recursive
```

## Commit policy

- One logical unit of work, one commit. For phases, that is "one phase,
  one commit" — or one commit per slice where PLAN.md says "commit per
  entity group". For chores, "one chore, one commit."
- Use Conventional Commits style:
  - `phase(N): <short description>` — phase work
  - `chore(<scope>): <short description>` — anything else
  - `fix(<scope>): <short description>` — bug fix
  - `docs(<scope>): <short description>` — doc-only change
- Include a longer body explaining *why*. Cross-reference the phase or
  task doc that drove the work.
- Commit message body uses HEREDOC to preserve formatting. The
  co-author trailer goes at the end. Use the attribution line your
  harness provides, if it provides one; otherwise:

  ```sh
  git commit -m "$(cat <<'EOF'
  phase(N): <short description>

  <longer body>

  Co-Authored-By: Claude <noreply@anthropic.com>
  EOF
  )"
  ```

- **Only commit when explicitly asked. Do not push unless explicitly
  asked. Never force-push.**

## GitHub comments — identify the LLM

When posting on GitHub via `gh` (PR comments, issue comments, review
replies) you are authenticating as the human repo owner. **Lead every
comment** with a one-line attribution naming the model that wrote it:

```
> _Posted by <model name and version> via gh CLI (authenticated as the repo owner)._
```

- Use the actual model name and version you are running as. If unsure,
  default to "Claude" and the model family.
- Applies to: `gh pr comment`, `gh pr review`, `gh issue comment`,
  inline review-thread replies via `gh api .../comments/{id}/replies`,
  and anywhere else a comment is created under the owner's identity.
- Does **not** apply to commit messages or PR descriptions.

## Resolving review threads — reply + resolve is one operation

After replying to an inline PR review comment, **also resolve the
underlying review thread** (GraphQL `resolveReviewThread`) in the same
workflow.

- Find the thread ID for a comment you replied to:

  ```sh
  gh api graphql -f query='{ repository(owner:"<owner>", name:"<repo>") {
    pullRequest(number:<N>) { reviewThreads(first:100) {
      nodes { id isResolved comments(first:1) { nodes { databaseId } } } } } } }'
  ```

  Match `databaseId` against the comment ID you replied to; the
  enclosing node's `id` is the thread ID.

- Resolve it:

  ```sh
  gh api graphql -f query='mutation { resolveReviewThread(input: {
    threadId: "<thread_id>" }) { thread { id isResolved } } }'
  ```

- **Exception**: if your reply declines the suggestion ("not fixed
  because <reason>"), leave the thread open so the reviewer can push
  back.

## PGP signing — non-negotiable

The user has commit signing configured and may require a passphrase.

- **Never** use `--no-gpg-sign`, `-S` with a hardcoded key,
  `--no-verify`, or any flag that bypasses signing or hooks.
- Attempt the commit normally. If it fails because of a passphrase
  prompt, signing key issue, or anything signing-related:
  1. Stop. Do not retry with workarounds.
  2. Tell the user the GPG cache likely needs warming:
     ```sh
     echo unlock | gpg --clearsign --local-user 5E42037421211E9D > /dev/null
     ```
  3. List the staged files and the exact commit message, so the user
     can either tell you "done, retry" or run it themselves.
  4. Wait. Do not proceed past the commit.
- Same applies to `git push` if push signing is configured.

## Guardrails (enforced on every commit)

All four must be green before you commit. Run them yourself; do not
assume.

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo deny check
```

Additional invariants enforced in code:

- `#![forbid(unsafe_code)]` at the top of every `lib.rs` and `main.rs`.
  No `unsafe` blocks anywhere, ever. If you think you need one, stop
  and log the reason in PROGRESS.md before continuing.
- If a lint cannot be resolved without compromising design, use
  `#[expect(<lint>, reason = "...")]` (never bare `#[allow]`) on the
  smallest possible scope and log the decision in PROGRESS.md. The
  workspace warns on `clippy::allow_attributes` and
  `clippy::allow_attributes_without_reason`; the only standing
  `#![allow]` is the generated-code module.

## Progress logging

Maintain `PROGRESS.md` at the repo root. Create it on first run.
Append a new entry whenever you finish a phase, complete a chore,
deviate from a plan, or hit something that surprises you. One entry,
one decision (do not batch).

### Entry format

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

Use a real timestamp captured at the moment you write the entry. The
timezone offset is part of the format:

```sh
date +"%Y-%m-%d %H:%M %z"
# 2026-10-08 10:55 +0800
```

### Commit timing for PROGRESS.md

The PROGRESS.md entry for phase N is committed at the *start* of phase
N+1, not in the same commit as the work it describes. This keeps each
phase commit clean and topical. The final phase's log entry goes in its
own follow-up commit.

For chores (not phases), commit the PROGRESS entry together with the
chore — there is no "next phase" to bundle it with.

PROGRESS.md is history. Never rewrite an old entry to match a later
rename; add a new entry that records the change.

## Invariants you must preserve

These are non-negotiable across every phase and chore.

1. **Single source of truth for the spec version.** The Network spec
   version lives in `crates/ferro-network/build.rs::SPEC_VERSION` and
   nowhere else in code. The spec path is derived from it. To bump,
   use `scripts/update-spec`; never hand-edit the constant in a change
   made for an unrelated reason.

2. **Hand-written code never names `crate::generated::...` types.**
   Every type that crosses a public signature is re-exported (and,
   where it helps, renamed) in `crates/ferro-network/src/models.rs`.
   When a spec bump renames a type, `models.rs` is the first (often
   only) fix-site.

3. **Wrappers are mechanical.** Each entity wrapper method
   (`DevicesApi::list`, `NetworksApi::get`, …) is a one-liner over the
   shared HTTP helpers on `NetworkClient` (`get_json`, `post_json`,
   `put_json`, `patch_json`, `delete`, and the paginated-list helper)
   and returns a `models::*` type. Never construct request URLs, query
   strings, or bodies inline in business logic — keep that mechanical
   layer in the helpers so every endpoint stays uniform.

4. **Request bodies use generated types where the spec defines them.**
   Where the spec only exposes a free-form schema, define a
   hand-written builder with
   `#[serde(skip_serializing_if = "Option::is_none")]` and add a
   comment pointing back to the spec path.

5. **API keys live in `SecretString` end to end.** From flag value to
   builder field to `HeaderValue` (with `set_sensitive(true)`). Never
   plain `String`.

6. **`UNIFI_NETWORK_*` env vars are forbidden in CI.** The CI workflow
   refuses to run if any are present. Both the CLI and live tests read
   this prefix; their presence in a CI runner would silently hit a
   real UniFi console / Network application.

If a task tempts you to violate one of these, stop. Either reshape the
task or log the reason for the deviation in PROGRESS.md and surface it
to the user before proceeding.

## Testing strategy

Cross-cutting: every endpoint added in any phase follows this.

### What every endpoint ships

1. **Mocked library test** at `crates/ferro-network/tests/<entity>.rs`
   using `wiremock`. Happy path with a committed JSON fixture under
   `tests/fixtures/`, plus the most relevant error path (401 for reads,
   404 for `get` endpoints, etc.).
2. **End-to-end CLI test** at `crates/ferro-network-cli/tests/<entity>.rs`
   using `assert_cmd`. Spawns the binary against a wiremock server.
   Assert exit code, human stdout, and `--json` stdout. Wrap the
   `Command` invocation in `tokio::task::spawn_blocking` so the sync
   `assert_cmd::Command::assert` does not block the Tokio reactor
   hosting the mock.
3. **Live test** at `crates/ferro-network/tests/live.rs` that runs
   against a real Network application. **Not** `#[ignore]`d — it checks
   env vars at the top of the function and skips cleanly when absent.

### Live test env-var contract

All vars share the `UNIFI_NETWORK_` prefix with the CLI. A single
sourced `.env.local` drives both the CLI and the live tests.

- `UNIFI_NETWORK_HOST` — hostname or `host:port`, no scheme prefix.
  **Required.** Absence means all live tests skip.
- `UNIFI_NETWORK_API_KEY_FILE` or `UNIFI_NETWORK_API_KEY` — at least
  one required when `HOST` is set. File path or raw key.
- `UNIFI_NETWORK_INSECURE` — non-empty/truthy ⇒ accept self-signed TLS.
- `UNIFI_NETWORK_SITE` — site to run site-scoped live tests against
  (UUID or `internalReference`). Default: the site whose
  `internalReference` is `default`, else the first site listed.
- `UNIFI_NETWORK_ALLOW_MUTATIONS=1` — permits `live_write_*` tests.
  Test gate only; the CLI never reads it.

If `HOST` is set but no key source is, the test helper **panics** with
a clear message rather than silently skipping. A half-configured live
env is almost always a developer mistake.

### Test naming convention

- `live_read_*` — non-mutating. Skip when `HOST` absent.
- `live_write_*` — mutating. Skip when `HOST` absent **or** when
  `UNIFI_NETWORK_ALLOW_MUTATIONS=1` absent.

Shared helpers in `crates/ferro-network/tests/common/mod.rs`:

```rust
pub fn live_client() -> Option<NetworkClient>;
pub async fn live_site_id(client: &NetworkClient) -> SiteId;
pub fn mutations_allowed() -> bool;
```

Live tests start with:

```rust
let Some(client) = common::live_client() else { return };
if !common::mutations_allowed() { return; } // live_write_* only
```

### Network mutations have a high blast radius

Network writes can cut off the machine running the tests (networks,
firewall, ACLs, WiFi, device restarts, PoE power-cycles). Every
`live_write_*` test must:

- target only resources the test itself created, where the API allows;
- restore any pre-existing value it changed, even on assertion failure
  (use a guard/cleanup pattern, not straight-line code);
- never touch the network, SSID, zone, or port the test host depends on.

**Never run mutating live tests against a production site.**

### Helper script

`scripts/live-test` sources `.env.local` (gitignored) and runs the live
tests with `--features insecure-tls` and `--nocapture`. Agents can set
env vars directly and skip the script.

### Insta snapshots — narrow scope

`insta` is used **only** for outputs of deterministic, pure
transformations:

- CLI `--help` text for the root command and each subcommand (phase 8).
- Canonical, stable error message formatting (phase 8).
- Codegen seam smoke tests, if a rewrite rule is complex enough to
  warrant one.

`insta` is **not** used for response bodies. Mocked tests assert
specific fields. Live tests assert structural properties.

## Logging conventions

- **Library (`ferro-network`)** emits through the
  [`log`](https://docs.rs/log) facade. Never initialises a logger.
- **CLI (`ferro-network-cli`)** wires `env_logger` in
  `crates/ferro-network-cli/src/logging.rs`. Filter precedence:
  `--log-level` flag > `UNIFI_NETWORK_LOG` env > `RUST_LOG` env >
  literal default `warn`. Output to **stderr** so `--json` and human
  tables on stdout stay parseable.
- Levels emitted in library code:
  - `info!` — top-level request outcome with cardinality ("listed 12
    devices"), `NetworkClient` construction with TLS mode label.
  - `debug!` — breadcrumb at every request entry (`GET /v1/...`),
    pagination progress, timeouts at builder time.
  - `warn!` — unexpected fallback paths (unexpected error-body shape,
    unknown error code).
- **Do not log API keys, raw request bodies, or response bodies in
  full.** Counts, ids, status codes, request IDs, and version strings
  are fine.

## Architecture documentation maintenance

[ARCHITECTURE.md](ARCHITECTURE.md) is a *living document* — the
"start here" for a human or agent who just cloned the repo.

- Update it whenever a phase or chore changes a structural decision,
  adds a new module category, or introduces a new invariant. Adding the
  eleventh wrapper does not change the architecture; adding the
  pagination stream does.
- Keep it tight. Target ~350 lines. Push detail to code or other docs.
- When adding a new top-level module or test pattern, update the file
  map in the same commit.
- Phase 8's sweep verifies the document still matches reality before
  tagging 0.1.0.

## Working style

- Prefer many small, well-tested changes over big sweeping ones.
- Every phase or chore ends with the four gates green before commit.
- When a task says "library + CLI + tests", deliver all three before
  it is done. No half-phases.
- If a plan is unclear or the spec contradicts the plan, pick the most
  defensible interpretation, log it in PROGRESS.md, and keep going. Do
  not block on small clarifications.
- Non-blocking review findings go into PLAN.md's "Deferred — revisit
  before 0.1.0" section with a **trigger condition**, not into a TODO
  comment.

### Prefer stdlib traits over bespoke `as_*` / `to_*` helpers

- Canonical string form → `impl Display`.
- Canonical conversion to another type → `impl From<Foo> for Bar`.
- Canonical parse from a string → `impl FromStr`.

Reserve bespoke methods for types with **several** distinct forms (wire
vs. human vs. log), where picking one as `Display` would be ambiguous.

### Don't repeat shared precedence in clap field docs

clap field doc-comments become `--help` text. Most global flags follow
the same "flag > env > config file > default" chain; summarise it
**once** in the `Cli` struct docstring (a `# Global option resolution`
section) and only spell out per-flag rules on flags that **deviate**
(`--api-key-file`, `--config`, `--log-level`, `--site`).

### Push back on low-utility, high-LOC features

Before saying "yes" and writing code, estimate the trade-off:

- **End-user utility**: who benefits, in what scenario, how often?
  Would an existing tool (`jq`, `$EDITOR`, `--help`) cover most of it?
- **LOC cost**: code + tests + docs + new dependencies. New deps
  amplify the cost (supply chain, compile time, `cargo deny` review).
- **Risk surface**: does it touch secrets, files, network config, or
  other hard-to-recover state?

If utility is low *and* LOC is high *and/or* risk is non-trivial, say
so concretely ("~N lines plus dep X; `--filter` plus `jq` covers 90%")
and offer an alternative. The human decides — but surface the
trade-off *before* writing the code, not after.
