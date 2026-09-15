# PR #191: exported-macro and split-visibility boundary increment

Date: 2026-09-15. Tracking: PR #191 / issue #187.

## Corrected escapes

The lexical Qdrant boundary already rejected raw SDK references, imported aliases,
private type aliases, multiline public signatures, public traits and public enums.
Two valid Rust layouts could still avoid the public-surface classification:

1. `#[macro_export] macro_rules!` and `pub macro` definitions could emit a vendor
   type without containing a `pub fn`/type/trait surface;
2. visibility and function qualifiers split across lines (`pub`, `async`,
   `unsafe`, `fn`) were not recognized as one public signature.

The source guard now tracks exported macro token trees with balanced `{}`, `()`
and `[]` delimiters, preserves the attribute line as the stable violation
location, follows imported vendor aliases inside the macro body, and recognizes
split public visibility/qualifier layouts. Private `macro_rules!` definitions
remain valid adapter internals.

## Regression inventory

Three focused tests cover direct/imported vendor types in `#[macro_export]`, a
`pub macro` definition, private-macro non-leakage, and a public function whose
visibility and qualifiers are split across lines. Existing source tests remain.

## Verification boundary

Required commands:

```text
cargo +1.98.0 test --locked -p xtask --lib qdrant_boundary::source::tests
cargo +1.98.0 run --locked -p xtask -- validate qdrant-boundary --json
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked -p xtask --all-targets -- -D warnings
```

Execution status in the current environment: **NOT_RUN** (`cargo` is absent).
This remains a lexical guard: it does not perform arbitrary macro expansion or
cross-file Rust name resolution, and it does not replace live Qdrant
qualification or independent review. No PASS, gate or task closure is claimed.
