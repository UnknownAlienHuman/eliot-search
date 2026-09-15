# T41 required-tooling completion boundary

Date: 2026-09-15. Tracking: PR #192 / issue #188.

## Repository state

The default branch contains no Python source files. Every retired mandatory
Python validator named by the migration inventory has one Rust `xtask` owner,
and its PowerShell compatibility wrapper invokes a locked Cargo command.
Required workflows remain `workflow_dispatch`-only and read-only.

This increment closes two remaining structural gaps:

1. a repository-wide regression test now walks every executable file under
   `tools/` and `.github/workflows/`, rejecting Python/Node-family sources,
   runtime manifests, executable commands and symbolic links rather than
   protecting only a hand-written subset of wrappers;
2. user-facing documentation no longer claims that active development
   validators still require Python.

The gate is intentionally limited to required repository tooling. It does not
ban an independently qualified future optional provider selected through its
own ADR and gate, and it grants no launch, package, gate or wave authority.

## Required execution

```text
cargo +1.98.0 test --locked -p xtask --test tooling_runtime_boundary --test tooling_runtime_inventory
cargo +1.98.0 check --locked -p xtask --all-targets
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked -p xtask --all-targets -- -D warnings
```

Execution status in the current environment: **NOT_RUN** (`cargo` is absent).
No compilation, test, formatting, Clippy, qualification or independent-review
PASS is claimed by this document.
