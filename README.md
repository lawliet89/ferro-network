# ferro-network

Async Rust client and CLI for the [UniFi Network](https://ui.com/) local
integration API. Targets Network application version **11.0.81**. The
workspace contains two crates: the `ferro-network` library and the
`ferro-network-cli` binary (`ferro-network`). Neither is published to
crates.io yet; build from a clone.

## Status

Pre-0.1.0. Built phase by phase (see [PLAN.md](PLAN.md)) against the
OpenAPI spec hosted at <https://github.com/beezly/unifi-apis>. A sibling
of [ferro-protect](https://github.com/lawliet89/ferro-protect).

New to the codebase? Start with [ARCHITECTURE.md](ARCHITECTURE.md).

## Clone

```sh
git clone --recurse-submodules https://github.com/lawliet89/ferro-network.git
```

If you forgot `--recurse-submodules`:

```sh
git submodule update --init --recursive
```

## Development

The local pre-commit hook runs `cargo fmt --check` and `cargo clippy`:

```sh
ln -s ../../scripts/pre-commit .git/hooks/pre-commit
```

## Running tests

### Quick start

```sh
cargo test --all
```

Runs everything: unit tests, mocked integration tests (against an
in-process `wiremock` server), doc tests, and live tests. Live tests
**skip themselves** when no console is configured, so this is safe on any
machine.

### Live tests

One-time setup:

```sh
cp .env.example .env.local
$EDITOR .env.local            # UNIFI_NETWORK_HOST + the API key file path
chmod 600 <your API key file>
```

Then:

```sh
./scripts/live-test           # optional test-name filter, e.g. live_read_info
```

The script sources `.env.local` and runs the live tests with
`--features insecure-tls` (so `UNIFI_NETWORK_INSECURE=1` works against a
self-signed console) and `--nocapture`. Or by hand:

```sh
set -a; source .env.local; set +a
cargo test --features insecure-tls --all
```

To poke at the real console with the CLI:

```sh
set -a; source .env.local; set +a
cargo run -p ferro-network-cli -- \
  --host "$UNIFI_NETWORK_HOST" \
  --api-key-file "$UNIFI_NETWORK_API_KEY_FILE" \
  ${UNIFI_NETWORK_INSECURE:+--insecure} \
  info
```

#### Environment variables

All prefixed `UNIFI_NETWORK_`, so nothing activates by accident:

| Var | Purpose |
|---|---|
| `UNIFI_NETWORK_HOST` | Console hostname or `host:port`, **no scheme**. The client builds `https://{host}/proxy/network/integration`. |
| `UNIFI_NETWORK_BASE_URL` | Instead of `HOST`: the full base URL (self-hosted Network application, or the Site Manager cloud connector, see `.env.example`). |
| `UNIFI_NETWORK_API_KEY_FILE` | Path to a file containing the API key. **Preferred.** |
| `UNIFI_NETWORK_API_KEY` | Raw API key, if a file is impractical. |
| `UNIFI_NETWORK_INSECURE` | Truthy (`1`, `true`, ...) skips TLS verification. Off in `.env.example`; opt in only for a self-signed certificate you cannot pin. |
| `UNIFI_NETWORK_SITE` | Site for site-scoped live tests (from phase 4). |
| `UNIFI_NETWORK_ALLOW_MUTATIONS` | `1` also runs `live_write_*` tests (from phase 6). |
| `UNIFI_NETWORK_LOG` | CLI log filter (env_logger syntax). Overridden by `--log-level`; falls back to `RUST_LOG`, then `warn`. Logs go to stderr. |

If a target is set but no key source is, the test helper panics rather
than silently skipping: a half-configured environment is almost always a
mistake.

> **Warning**: mutating live tests (`UNIFI_NETWORK_ALLOW_MUTATIONS=1`)
> change real network configuration and can cut off the machine running
> them. **Never run mutating live tests against a production site.**

### Security notes

- `.env` and `.env.local` are gitignored. Never commit them.
- Prefer `UNIFI_NETWORK_API_KEY_FILE` over the raw variable, keep the key
  file outside the repository, and `chmod 600` it.
- CI fails fast if any `UNIFI_NETWORK_*` variable is present in the
  runner, so a leaked secret cannot reach a real console from a PR build.

## License

Apache-2.0. See [LICENSE.md](LICENSE.md).
