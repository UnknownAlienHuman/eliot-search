# Historical product-status records

The files in this directory are historical snapshots from the Architecture 8.4 package/wave programme.
They are not current implementation, launch, readiness or scheduling authority.

- `PACKAGE_STATUS.toml` must not be regenerated, updated or consulted to decide which package may change.
  The current target disposition for all 52 workspace members is
  [`docs/audit/WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md`](../audit/WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md).
- `AGENT_ANALYSIS_FRAMEWORK.md` may contain useful review heuristics, but its packet/gate/wave/receipt model
  cannot authorize or block work.

Current project direction is:

1. [`docs/architecture/README.md`](../architecture/README.md)
2. [`docs/implementation/README.md`](../implementation/README.md)
3. [`docs/audit/AUDIT_COVERAGE_AND_DOCUMENTATION_MAP_2026-10-21.md`](../audit/AUDIT_COVERAGE_AND_DOCUMENTATION_MAP_2026-10-21.md)
4. the current package owner docs and active GitHub issue.

Git history is the archive. Delete these historical files with their final tooling/document consumer; do not
replace them with another status registry.
