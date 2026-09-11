# Rust-only validator entrypoint boundary

This qualification freezes validator migrations already completed under T41. It does not claim that every optional development helper has been ported yet.

Run:

```powershell
cargo test --locked -p xtask --test tooling_runtime_boundary
```

or the compatibility wrapper:

```powershell
pwsh -NoProfile -File tools/validate-rust-only-entrypoints.ps1
```

The gate requires every migrated wrapper to invoke the locked `xtask` Rust command, rejects Python/Node invocations, rejects restoration of retired `.py` implementations, and rejects workflow references to those files.

A passing run prevents regression of completed slices. It does not issue authority, qualify Qdrant or prove the remaining T41 backlog complete.
