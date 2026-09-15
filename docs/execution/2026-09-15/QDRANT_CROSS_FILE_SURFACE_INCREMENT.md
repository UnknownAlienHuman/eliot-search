# PR #191: cross-file Qdrant surface increment

Date: 2026-09-15. Tracking: PR #191 / issue #187.

## Corrected escape

The existing lexical guard resolved vendor imports and private aliases only
inside one Rust file. A bridge module could therefore define a restricted alias
for a Qdrant SDK type and another module could publicly re-export that alias or
import it into a public signature without containing the raw vendor path itself.

The `source/cross_file.rs` pass retains bridge `src/**.rs` files from the
existing bounded repository walk, maps Rust source files to semantic module
paths, identifies vendor-tainted imports/type aliases and propagates taint
through public `use` chains to a fixed point. It reports:

- direct and renamed public item re-exports;
- grouped item re-exports;
- chained facade/root re-exports;
- renamed `extern crate` roots;
- private cross-file imports used by public signatures.

Taint keys include the full module path and item name. An unrelated bridge-owned
item with the same leaf name in another module is not rejected.

## Follow-up closure

The later
`QDRANT_MODULE_GRAPH_SURFACE_INCREMENT.md` slice extends this owner with:

- direct literal `include!` module inheritance;
- direct literal `#[path] mod` overrides;
- public and private glob reachability;
- local type aliases derived from cross-file imports;
- exportability-aware public glob propagation;
- fail-closed rejection of direct vendor-SDK glob imports.

The combined pass remains lexical and bounded. Direct literal module targets
must resolve inside the retained inventory; unresolved, computed, cyclic or
oversized direct graphs and malformed or oversized import trees fail closed.
Macro-generated directives, `cfg_attr` selection, module aliases used as path
roots, conditional-compilation truth and compiler-derived reachability
remain compiled-check and independent-review obligations.

No Qdrant dependency/version, production adapter, manifest schema, workflow,
qualification receipt or authority record changed.

## Regression inventory

Unit tests cover direct/renamed/grouped/chained item re-exports, renamed
`extern crate`, public-signature imports, module-graph overrides, glob imports,
local alias chains, unrelated same-name isolation and internal-only bindings.

`qdrant_cross_file_boundary.rs` exercises the public validator on disposable
synthetic repository fixtures for positive and negative cases.

## Required execution

```text
cargo +1.98.0 test --locked -p xtask --lib \
  qdrant_boundary::source::cross_file::tests \
  qdrant_boundary::source::module_graph::tests
cargo +1.98.0 test --locked -p xtask --test qdrant_cross_file_boundary
cargo +1.98.0 test --locked -p xtask --test qdrant_boundary_regressions
cargo +1.98.0 run --locked -p xtask -- validate qdrant-boundary --json
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked -p xtask --all-targets -- -D warnings
```

Execution status in the current environment: **NOT_RUN** (`cargo` is absent).
No structural PASS, live qualification, task closure or independent review is
claimed.
