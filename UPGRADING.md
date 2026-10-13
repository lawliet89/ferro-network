# Upgrading the UniFi Network spec version

ferro-network pins one OpenAPI spec version at a time: the `SPEC_VERSION`
constant in [`crates/ferro-network/build.rs`](crates/ferro-network/build.rs).
The spec file comes from the `third_party/unifi-apis` submodule. Bumping it
re-runs typify over `components.schemas`, so breaking model or wrapper
changes show up at compile time. This document covers the happy path and
the things that go wrong.

## Happy path

```sh
./scripts/update-spec               # lists versions in the checked-out submodule
./scripts/update-spec 11.1.42       # fetches, bumps, regenerates, runs all four gates
```

With a version argument the script:

1. Fetches `third_party/unifi-apis` and checks out `origin/HEAD`; the
   requested version must exist there.
2. Rewrites the `SPEC_VERSION` line in `build.rs` (and touches it so the
   build script re-runs) and the version claim in `README.md`.
3. Runs `cargo build`, `fmt --check`, `clippy`, `test --all`, `deny check`.
4. Prints the new submodule SHA and the `git add` / `git commit` to run.

If every step passes, commit the submodule, `build.rs`, and `README.md`
together.
Step one version at a time, newest minor first.

## When codegen fails

Build-script failures read `ferro-network codegen failed: ...`. The build
script parses the spec, runs `build_support/spec_rewrite.rs::rewrite`
(pure, structural JSON Schema preprocessing), and feeds
`components.schemas` to typify. Current rules, in pipeline order:

- **`coerce_numeric_string_enums`**: `type: integer` with `enum: ["1000"]`
  becomes `enum: [1000]`. Without it typify fails with *"value does not
  conform to the given schema"*.
- **`disambiguate_case_colliding_names`**: schema names that collide as
  Rust identifiers (`IP Address selector` / `IP address selector`) get a
  ` 2` suffix. Symptom without it: `the name X is defined multiple times`.
- **`lift_discriminators_to_one_of`**: `discriminator` bases become
  `oneOf` unions over their mapping targets, each target pinned to its
  tag. It skips unions whose tag property `enum` contradicts the mapping
  keys. Symptom of a gap: a union emitted as a plain struct with a
  `String` tag.
- **`unconstrained_properties_to_true`**: annotation-only property
  schemas become `true`, so typify keeps named union types when merging
  `allOf`. Symptom of a gap: types named `...Variant0`, `...Variant1`.

Triage order:

1. typify error: find the schema with a throwaway debug build. Print
   `name` in the `build.rs` loop, or run typify with `RUST_LOG=info` in a
   scratch crate: it logs each schema as it converts it.
2. Prefer a narrow structural rule in `spec_rewrite.rs`, with a doc
   comment naming the schemas that need it and a unit test in
   `tests/model_codegen.rs`. Never match on schema names.
3. If one schema is hopeless, hand-write that type in `models.rs` and
   drop the schema from codegen with a small, commented skip list in
   `build.rs`.

## When wrappers fail to compile

1. **`crates/ferro-network/src/models.rs`**: the seam. A renamed or
   reshaped generated type breaks a re-export here first. Fix it here.
2. **The entity module**: wrappers are one-liners over shared helpers; a
   changed path only needs its string updated.
3. **`crates/ferro-network/tests/model_codegen.rs`**: names every
   re-export, so a missing type fails here with a clear pointer.

Hand-written code never names `crate::generated::...`. If you need a new
generated type, re-export it from `models.rs` first.

## Reading the generated diff

The generated source is
`target/debug/build/ferro-network-*/out/generated.rs` (pick the newest
if there are several):

```sh
cp "$(ls -t target/debug/build/ferro-network-*/out/generated.rs | head -1)" /tmp/generated.before
./scripts/update-spec <new-version>
cp "$(ls -t target/debug/build/ferro-network-*/out/generated.rs | head -1)" /tmp/generated.after
diff -u /tmp/generated.before /tmp/generated.after | less
```

Watch for unions that became structs, new `VariantN` types, and fields
that became `serde_json::Value`.

## Agent checklist

1. Read this file and the newest PROGRESS.md entries.
2. Run `./scripts/update-spec` (no args) and pick the next version.
3. Run `./scripts/update-spec <version>`. If it exits 0, commit
   (`chore(spec): bump Network spec to <version>`). Done.
4. Build-script failure: classify it with "When codegen fails". Add or
   adjust a rewrite rule (with a test), then go back to step 3.
5. Compile failure in hand-written code: start at `models.rs`, then the
   wrappers.
6. Test failure: read the assertion. A fixture that no longer matches
   the spec gets updated alongside the bump.
7. `cargo deny` failure: a dependency changed. Update `deny.toml` with a
   reason.
8. Log the bump and any new rule in PROGRESS.md.

Do not bypass signing, do not edit `target/`, do not amend prior commits.
