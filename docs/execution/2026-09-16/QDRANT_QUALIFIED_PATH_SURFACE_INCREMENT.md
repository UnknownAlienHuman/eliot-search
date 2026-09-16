# PR #191: direct qualified-path Qdrant surface increment

Date: 2026-09-16. Tracking: PR #191 / issue #187.

## Corrected escape

The cross-file guard already propagated taint through imports, aliases, globs,
`include!` and direct `#[path]` module overrides. A public surface could still
name a restricted vendor alias without any import:

```rust
pub fn client() -> crate::private::VendorClient;
```

That path contains no locally visible alias for the existing identifier matcher
to follow. The new bounded qualified-path scanner resolves `crate`, `self`,
repeated `super`, bare module paths, raw identifiers and `$crate` roots against
the same semantic module graph and module-qualified taint map.

Public signatures, trait/enum bodies and exported macro token trees are now
accumulated before matching. Splitting `crate::private::VendorClient` across
lines does not hide it. Every prefix of an associated path is checked, so
`crate::private::VendorClient::Associated` is rejected when the vendor alias is
the tainted prefix.

The scanner is bounded to 64 segments per path and 65,536 qualified paths per
public surface. Exceeding either bound fails closed. Private qualified paths
remain internal, and `crate::owned::VendorClient` remains valid when only
`crate::private::VendorClient` is tainted.

## Regression inventory

The focused parser tests cover:

- crate-relative, bare, raw-identifier and line-split paths;
- `$crate` exported-macro paths;
- repeated relative resolution through `super`;
- tainted prefixes before associated items;
- unrelated module-qualified same-name types.

The disposable-repository integration test covers direct public function
returns, multiline enum variants, exported macros, nested `super` paths and the
negative same-name/private-use cases.

No Qdrant dependency/version, production adapter, manifest schema, workflow,
qualification receipt or authority record changed.

## Remaining boundary

This is still a bounded lexical structural gate, not the Rust compiler. Dynamic
macro expansion, `cfg_attr` module selection, conditional-compilation truth,
module aliases used as path roots and compiler-derived effective visibility
remain complementary compiled-check and independent-review obligations.

## Required execution

```text
cargo +1.98.0 test --locked -p xtask --lib \
  qdrant_boundary::source::cross_file::qualified_path::tests
cargo +1.98.0 test --locked -p xtask --test qdrant_qualified_path_boundary
cargo +1.98.0 test --locked -p xtask --test qdrant_cross_file_boundary
cargo +1.98.0 test --locked -p xtask --test qdrant_boundary_regressions
cargo +1.98.0 run --locked -p xtask -- validate qdrant-boundary --json
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked -p xtask --all-targets -- -D warnings
```

Execution status in the current environment: **NOT_RUN** (`cargo`, `rustc` and
`rustfmt` are absent). No structural PASS, live qualification, task closure or
independent review is claimed.
