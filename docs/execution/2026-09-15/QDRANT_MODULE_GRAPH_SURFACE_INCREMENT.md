# PR #191: Qdrant module-graph and glob boundary increment

Date: 2026-09-15. Tracking: PR #191 / issue #187.

## Corrected escapes

The cross-file scanner previously assigned every bridge source only its
canonical filesystem-derived module path and ignored glob imports. That left
four lexical escape classes:

- a vendor alias declared in a literal `include!` file and consumed by the
  including module;
- a direct `#[path = "..."] mod name;` override whose semantic module name
  differs from its filename;
- `pub use module::*` leaking a public vendor-tainted item;
- `use module::*` importing a vendor-tainted item into a public signature.

The scanner now builds a bounded semantic module graph over the already retained
bridge `src/**.rs` inventory. It preserves canonical module identities and adds
fixed-point edges for direct literal `include!` and `#[path] mod` declarations.
No filesystem path outside that inventory is read and no macro is executed.
Literal targets must resolve inside the retained inventory. Missing targets,
direct computed targets, include cycles, excessive semantic identity growth,
excessive module depth and malformed or oversized import trees fail closed
rather than returning a partial result.

Cross-file taint now propagates through normal, grouped and glob imports.
Public globs use only publicly exportable taint, preventing an internal-only SDK
binding from becoming a false public leak. Private globs retain all visible
taint for public-signature checks. Direct SDK glob imports fail closed because
introduced vendor names cannot be enumerated lexically. Local type aliases are
expanded after cross-file imports, closing
`foreign alias -> local alias -> public signature`.

## Regression inventory

Unit and disposable-repository tests cover:

- literal include inheritance;
- direct path-attribute module identity;
- inert comment/string directives;
- missing/computed targets and include cycles failing closed;
- public glob re-export and direct SDK glob rejection;
- private glob use in a public signature;
- internal-only vendor bindings under a public glob;
- cross-file import through a local alias;
- existing renamed/grouped/chained re-export and same-name isolation cases.

## Retained limitations

This is still a bounded lexical validator. Direct computed module paths are
rejected rather than executed or guessed. It does not evaluate `cfg`, expand
macros, resolve `cfg_attr` module selection, resolve local module aliases used
as path roots, or substitute for Cargo compilation and independent public-API
review. Unsupported macro-generated construction remains a hard review
condition rather than inferred PASS.

## Required execution

```text
cargo +1.98.0 test --locked -p xtask --lib \
  qdrant_boundary::source::module_graph::tests \
  qdrant_boundary::source::cross_file::tests
cargo +1.98.0 test --locked -p xtask --test qdrant_cross_file_boundary
cargo +1.98.0 test --locked -p xtask --test qdrant_boundary_regressions
cargo +1.98.0 run --locked -p xtask -- validate qdrant-boundary --json
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked -p xtask --all-targets -- -D warnings
```

Execution status in the current environment: **NOT_RUN** (`cargo` is absent).
No structural PASS, live qualification, task closure or independent review is
claimed.
