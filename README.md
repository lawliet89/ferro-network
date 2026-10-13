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

_Written in phase 2._

> **Warning**: mutating live tests (`UNIFI_NETWORK_ALLOW_MUTATIONS=1`)
> change real network configuration and can cut off the machine running
> them. **Never run mutating live tests against a production site.**

## License

Apache-2.0. See [LICENSE.md](LICENSE.md).
