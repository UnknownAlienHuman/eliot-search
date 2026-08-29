# Swarm control files

`crates.toml` is the machine-readable registry for one-agent/one-package implementation. It records 43
Cargo packages, path, earliest wave, optionality, line budget, assignment and direct dependencies. The
package's `${path}/AGENTS.md` plus `assignments/${package}.md` form the bounded implementation brief.

The orchestrator must:

1. select only packages authorized by `launch-state.toml`;
2. verify accepted handoffs and API digests for every direct dependency/consumed port;
3. create one immutable-base worktree per writer;
4. supply root/family/package instructions, assignment and accepted dependency notes;
5. reject writes outside `${path}/**`;
6. route contract changes through the integration owner;
7. merge in topological order and publish a wave receipt.

Files:

- [`crates.toml`](crates.toml) — package registry;
- [`launch-state.toml`](launch-state.toml) — current implementation authority;
- [`assignments/`](assignments/README.md) — one bounded packet per package;
- [`ASSIGNMENT_PROTOCOL.md`](ASSIGNMENT_PROTOCOL.md) — writer execution rules;
- [`CONTRACT_CHANGE_TEMPLATE.md`](CONTRACT_CHANGE_TEMPLATE.md) — missing/changed contract request;
- [`PACKAGE_HANDOFF_TEMPLATE.md`](PACKAGE_HANDOFF_TEMPLATE.md) — writer completion receipt;
- [`REVIEW_CHECKLIST.md`](REVIEW_CHECKLIST.md) — package and integration review;
- [`../docs/handoff/PORT_CATALOG.md`](../docs/handoff/PORT_CATALOG.md) — port/adapter ownership.
