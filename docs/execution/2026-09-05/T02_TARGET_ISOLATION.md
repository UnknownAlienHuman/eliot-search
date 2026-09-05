# T02 — exact first isolation slice, proposed for review

Source: `8ef226d8dca4368e2fe83c37c870f56190b2c168`. T01 preparation only; not T02 acceptance or a writer lease.

## Candidate patch

`T02_TARGET_ISOLATION.patch` changes exactly `bins/eliot-searchd/Cargo.toml` and `bins/eliot-search/Cargo.toml`:

- set `autobins = false` in both packages;
- retain the primary `eliot-searchd` and `eliot-search` bins at their existing `src/entry.rs` paths;
- reclassify the two snapshot bins and six auto-discovered sealed prototypes as explicit examples with `test = true`;
- preserve names, source paths, dependencies, features and lockfile; add no `required-features` escape hatch.

| Package | Experimental target | Existing source path |
|---|---|---|
| daemon | `eliot-search-snapshotd` | `src/main.rs` |
| daemon | `eliot-search-sealed-authority` | `src/bin/eliot-search-sealed-authority.rs` |
| daemon | `eliot-search-sealed-catalog` | `src/bin/eliot-search-sealed-catalog.rs` |
| daemon | `eliot-search-sealed-direct` | `src/bin/eliot-search-sealed-direct.rs` |
| daemon | `eliot-search-sealed-recover` | `src/bin/eliot-search-sealed-recover.rs` |
| daemon | `eliot-search-sealed-store` | `src/bin/eliot-search-sealed-store.rs` |
| daemon | `eliot-search-sealed-transaction` | `src/bin/eliot-search-sealed-transaction.rs` |
| CLI | `eliot-search-snapshot` | `src/main.rs` |

The sealed recovery entrypoint directly imports `sealed_digest`, `sealed_owner_epoch`, `sealed_recovery`, `sealed_root_lock`, `sealed_store`, `sealed_transaction` and `sealed_transaction_guard`. These regression-bearing modules must not disappear just because recovery stops being an ordinary installed binary. The source-path-preserving candidate avoids breaking its existing relative module paths.

Cargo documents `autobins`, explicit example paths and `test = true` in its [target reference](https://doc.rust-lang.org/cargo/reference/cargo-targets.html). Examples remain in the all-target verification surface; no green build is obtained by simply skipping a broken prototype. An example can still be built/run explicitly: this is packaging separation, not a security sandbox or process-ownership fix.

## Application boundary

The patch is a candidate input for T02, not an independently shippable completion of that task. Before applying it to the actual T02 branch, acquire its accepted prerequisites and review these exact companion obligations:

1. Find all active commands and `CARGO_BIN_EXE_*` references to the eight names. Update the manual core-test invocation of sealed recovery to `--example eliot-search-sealed-recover` and keep its executed test inventory nonzero. Record the exact workflow/document/test path additions to the T02 scope; do not edit historical logs or fabricate replacement runs.
2. Ensure explicitly run examples cannot share an authoritative product data root through a different lock. Reclassification alone does not solve the root-owner conflict. No new experimental format may silently initialize over a product root.
3. Verify default `cargo build --bins`, ordinary install/package contents and explicit example invocation. The two main packages must expose only their primary ordinary bin. Optional worker packages are a separate T42 concern.
4. Run all-target checks and the example tests on Linux and Windows; preserve DPAPI, epoch, transaction and recovery regressions. A failed or unavailable lane remains failed/unavailable.

Do not run a repository-wide move based on a filename prefix. In particular, `direct_store.rs` is the live plaintext catalog module behind `secure_direct_store.rs`, not a dead alternative that can be deleted with the snapshot code. Keep `#[path]` aliases and shared test closures intact until exact expanded reachability is captured.

## Size and remaining extraction

This first slice moves **zero source files** and reduces **zero handwritten source lines**. It cannot satisfy the package line budget. The full T02 extraction map still requires measured per-package line counts, complete module closure and accepted capability APIs. A mechanical `git mv` of live catalog/crypto/transport code into a capability package can introduce dependency cycles or duplicate owners; it is not automatically safe.

## Checks performed on the candidate

The two before-images matched their exact Git blob IDs. `git apply --check` and application to an isolated two-manifest fixture succeeded; parsed TOML retained dependencies/features, two primary bins and eight test-enabled examples. This is patch/manifest validation, not `cargo metadata`, compilation or native execution. Product manifests and CI remain unchanged in T01.
