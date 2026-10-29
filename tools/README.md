# Development commands

Use direct Cargo and Git commands for the affected production owner and its real immediate consumers:

```text
cargo +1.98.0 check --locked -p <owner> [-p <consumer> ...]
cargo +1.98.0 clippy --locked -p <owner> [-p <consumer> ...] -- -D warnings
git diff --check
```

Select actual binaries when their wiring changes. Run broad behavior, platform and scale qualification after the
connected product exists; do not use unrelated suites to qualify a documentation-only change.

The documentation landing does not remove main's old PowerShell/xtask tooling or change its workflows.
PR #610 contains separate removal work. Legacy packet, digest, wave and acceptance validators are not current
product requirements. Remove them with their real consumers instead of extending them or inventing replacements.

Source/verification status lives in the [registry](../docs/audit/ANALYSIS_REGISTRY.md#evidence-status).
