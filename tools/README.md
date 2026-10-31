# Development commands

Use [WORKING_GUIDE.md](../docs/implementation/WORKING_GUIDE.md#minimal-production-checks) for direct target-specific
Cargo commands and document updates. The manual GitHub workflow selects one package/target and runs check plus Clippy;
it does not run tests or interpret approval packets. Local execution is the normal route.

The old xtask, PowerShell validators, generated maps and wave workflows are removed. Do not recreate them to
validate directory shapes, package counts, plan digests or historical fixtures. Check behavior through its real consumer.
