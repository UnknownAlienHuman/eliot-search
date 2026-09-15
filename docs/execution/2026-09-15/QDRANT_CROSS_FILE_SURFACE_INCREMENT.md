# PR #191: cross-file Qdrant surface increment

Date: 2026-09-15. Tracking: PR #191 / issue #187.

## Corrected escape

The existing lexical guard resolved vendor imports and private aliases only
inside one Rust file. A bridge module could therefore define a restricted alias
for a Qdrant SDK type and another module could publicly re-export that alias or
import it into a public signature without containing the raw vendor path itself.

The new `source/cross_file.rs` pass retains bridge `src/**.rs` files from the
existing bounded repository walk, maps canonical Rust file paths to module
paths, identifies vendor-tainted imports/type aliases and propagates taint
through public `use` chains to a fixed point. It reports:

- direct and renamed public re-exports;
- grouped re-exports;
- chained facade/root re-exports;
- renamed `extern crate` roots;
- private cross-file imports used by public signatures.

Taint keys include the full module path and item name. An unrelated bridge-owned
item with the same leaf name in another module is not rejected.

## Scope retained

The pass is lexical and bounded. It recognizes canonical `lib.rs`, nested
`*.rs` and `mod.rs` layouts and resolves `crate`, `self`, repeated `super` and
Rust-2018 crate-root `use` paths. It does not attempt arbitrary macro expansion,
noncanonical `#[path]` graphs, public glob reachability or compiler-derived
reachability. Those remain compiled-check and independent-review obligations.

No Qdrant dependency/version, production adapter, manifest schema, workflow,
qualification receipt or authority record changed.

## Regression inventory

Unit tests cover:

- private alias re-export from the crate root;
- private import entering a public signature;
- grouped and chained re-exports;
- renamed `extern crate` propagation;
- unrelated same-name isolation;
- internal-only cross-file imports.

`qdrant_cross_file_boundary.rs` also exercises the public validator on disposable
synthetic repository fixtures for re-export, public-signature and negative
same-name cases.

## Required execution

```text
cargo +1.98.0 test --locked -p xtask --lib qdrant_boundary::source::cross_file::tests
cargo +1.98.0 test --locked -p xtask --test qdrant_cross_file_boundary
cargo +1.98.0 test --locked -p xtask --test qdrant_boundary_regressions
cargo +1.98.0 run --locked -p xtask -- validate qdrant-boundary --json
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked -p xtask --all-targets -- -D warnings
```

Execution status in the current environment: **NOT_RUN** (`cargo` is absent).
No structural PASS, live qualification, task closure or independent review is
claimed.
