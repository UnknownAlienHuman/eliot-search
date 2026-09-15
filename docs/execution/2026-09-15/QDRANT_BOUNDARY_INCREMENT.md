# PR #191 / issue #187: Qdrant boundary corrective increment

Date: 2026-09-15. Implementation base:
`17a2375af8eba65c50105031f2bc42bdb9c9119a` (`main`).
Scope: integration-owned `xtask` guard, its tests and boundary documentation.
The task PR remains tracking material; this is not an accepted handoff or gate.

## Findings corrected

| Existing failure | Correction |
|---|---|
| Dependency collection recognizes only the literal `qdrant-client` key. | Also inspect `package`, including renamed workspace and target/dev/build dependencies. |
| Matching version strings can hide a different dependency source or bridge feature set. | Reject Qdrant patch/replace entries and source selectors; bridge must inherit the workspace source/features. |
| Lockfile lookup returns the first matching client. | Require exactly one client record, even if duplicate records have equal versions. |
| SDK detection depends on contiguous text such as `qdrant_client::` and prefix-matches unrelated module names. | Stream complete tokens after masking inert text; recognize spacing, raw identifiers, grouped imports and extern-crate aliases. |
| Version lookup can read a fake declaration inside a block comment or raw string. | Require the declaration to occur in masked active code; reject duplicate canonical declarations. |

No dependency, lockfile, vendor version, production adapter, public report
schema, persistent format, workflow or authority record was changed. The
existing file-local public-surface implementation is retained, not replaced
with a compiler or a new dependency.

## Regression inventory

Five new source-unit tests cover spaced references, import forms, exact token
boundaries, inert version declarations and missing/duplicate declarations.
`xtask/tests/qdrant_boundary_regressions.rs` adds eleven tests through the public
validator on disposable synthetic repository fixtures: clean/read-only/repeated
validation; renamed dependencies; renamed workspace inheritance; patch/replace;
workspace source substitution; bridge overrides; duplicate lock records; spaced
SDK escape; comment-spoofed version; inert text/unrelated names; and existing
direct-dependency/public-surface rejection.

All original source tests remain. Fixture versions `1.2.3` and `2.3.4` are
synthetic parser input, not qualified product artifacts. The tests do not fetch,
install or start Qdrant and do not modify the real repository or issue receipts.

## Verification boundary

- Exact baseline contents for all four modified files were materialized from
  GitHub and checked against their Git blob SHA-1 before editing.
- `git diff --check` succeeded on the complete selected-file patch.
- Changed files, test inventory and fixture setup were inspected locally.
- Toolchain preflight `cargo +1.98.0 --version` exited **127**:
  `cargo: command not found`. No Rust compiler or formatter was available.
- Rust compilation, tests, rustfmt, strict Clippy, native Windows execution and
  live Qdrant checks are **NOT_RUN**. No runtime PASS or independent review is
  claimed. Local source checks do not substitute for those commands.

Required execution from a complete checkout, still outstanding:

```text
cargo +1.98.0 test --locked -p xtask --lib qdrant_boundary::source::tests
cargo +1.98.0 test --locked -p xtask --test qdrant_boundary_regressions
cargo +1.98.0 run --locked -p xtask -- validate qdrant-boundary --json
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked -p xtask --all-targets -- -D warnings
```

## Remaining PR #191 acceptance work

The existing surface/import analysis remains lexical and file-local. It does
not resolve cross-file aliases/re-exports or macro expansion, and its line-based
public-surface recognizer still needs adversarial layout coverage. The existing
filesystem walk/read helpers have no explicit aggregate entry/depth/byte budget
and skip symlinks; those require a separately tested fail-closed correction.
Canonical version extraction is intentionally format-restricted, not a Rust
constant evaluator. Complete boundary acceptance, exact-head execution and
independent review remain outstanding. Do not close the task or enable an
optional/live capability on the basis of this increment.
