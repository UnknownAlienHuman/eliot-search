# Swarm control files

The registry contains 44 one-writer packages: 40 libraries and 4 binaries. `launch-state.toml`, not
Cargo membership, decides what may run.

Foundation order:

```text
search-contracts
  ├─ search-domain
  └─ search-ports
```

Files:

- `crates.toml` — package paths, dependencies, assignments, waves and limits;
- `launch-state.toml` — active authorization and advancement conditions;
- `assignments/` — one bounded implementation packet per package;
- `ASSIGNMENT_PROTOCOL.md` — writer rules;
- `INTEGRATION_OWNER.md` — root/cross-package owner;
- `CONTRACT_CHANGE_TEMPLATE.md` — missing/changed contract or port request;
- `PACKAGE_HANDOFF_TEMPLATE.md` — completion receipt;
- `REVIEW_CHECKLIST.md` — package and integration review.

The orchestrator verifies accepted dependency API/port digests, creates isolated worktrees, rejects
out-of-scope writes and merges in topological order.
