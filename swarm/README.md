# Swarm control files

`crates.toml` is the machine-readable package registry. `launch-state.toml` is the only current
authorization to start implementation. `assignments/<package>.md` is the bounded implementation packet
for one writer and one Cargo package.

The orchestrator must:

1. read `launch-state.toml` and select only an authorized package;
2. verify accepted handoffs/public API digests for every direct dependency;
3. create one immutable-base worktree per writer;
4. provide root/family/package instructions, `ASSIGNMENT_PROTOCOL.md`, exactly one assignment and accepted public dependency notes;
5. reject writes outside the package path;
6. route contract changes through the contract owner;
7. merge in topological order and publish a wave receipt;
8. advance launch state only through the integration owner.

Ordinary writers do not read the architecture master, dependency internals, future-wave package briefs or
the entire final daemon graph.

Files:

- [`crates.toml`](crates.toml) — package path, wave, optionality, line budget and direct dependency registry;
- [`launch-state.toml`](launch-state.toml) — current authorized wave/package set;
- [`ASSIGNMENT_PROTOCOL.md`](ASSIGNMENT_PROTOCOL.md) — common writer/read/write/evidence rules;
- [`assignments/`](assignments/README.md) — one bounded package-specific packet per Cargo package;
- [`ASSIGNMENT_SCHEMA.md`](ASSIGNMENT_SCHEMA.md) — mandatory packet structure;
- [`INTEGRATION_OWNER.md`](INTEGRATION_OWNER.md) — root/cross-package authority;
- [`CONTRACT_CHANGE_TEMPLATE.md`](CONTRACT_CHANGE_TEMPLATE.md) — missing/changed contract request;
- [`PACKAGE_HANDOFF_TEMPLATE.md`](PACKAGE_HANDOFF_TEMPLATE.md) — writer completion receipt;
- [`REVIEW_CHECKLIST.md`](REVIEW_CHECKLIST.md) — package and integration review.
