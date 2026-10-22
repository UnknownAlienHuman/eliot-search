# Development commands

The repository has no current PowerShell control plane, packet validator, package-map generator or accepted-evidence wrapper. Use direct Cargo and Git commands against the actual code being changed.

For an implementation slice, check the changed production owner and its real immediate consumers:

```powershell
cargo +1.98.0 check --locked -p <owner> [-p <consumer> ...]
cargo +1.98.0 clippy --locked -p <owner> [-p <consumer> ...] -- -D warnings
git diff --check
```

Run focused behavior or live checks only for the mechanism actually connected by the slice. Windows durability, blocked-I/O cancellation, live Qdrant recovery, relevance, scale and installed-product claims require their own executed cases.

A broad workspace command is a late diagnostic, not a prerequisite for every refactor:

```powershell
cargo +1.98.0 metadata --locked --format-version 1 --no-deps
cargo +1.98.0 check --workspace --locked
```

The manual GitHub workflow records the same kind of raw diagnostic output and grants no package, gate, wave, readiness or release authority.

The legacy `xtask` package, generated maps and digest ledger were removed. Do not recreate a custom command merely to mirror Cargo metadata, source layout, review status or acceptance state. Add repository-specific tooling only when a surviving product workflow cannot be expressed clearly with direct Cargo or Git commands.
