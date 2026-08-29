# Swarm control files

`crates.toml` is the compact machine-readable registry for one-agent/one-package implementation. It
records package path, first authorized wave, optionality, line budget and direct dependencies. The
package's `${path}/AGENTS.md` is the complete bounded implementation brief; the full human matrix is
`docs/handoff/CRATE_MATRIX.md`.

The orchestrator must:

1. select only packages from the active wave;
2. verify accepted handoffs for every direct dependency;
3. create one immutable-base worktree per writer;
4. supply root/family/package instructions and direct dependency API notes;
5. reject writes outside `${path}/**`;
6. route contract changes through the contract owner;
7. merge in dependency order and publish a wave receipt.

Files:

- [`crates.toml`](crates.toml) — compact package registry;
- [`CONTRACT_CHANGE_TEMPLATE.md`](CONTRACT_CHANGE_TEMPLATE.md) — missing/changed contract request;
- [`PACKAGE_HANDOFF_TEMPLATE.md`](PACKAGE_HANDOFF_TEMPLATE.md) — writer completion receipt;
- [`REVIEW_CHECKLIST.md`](REVIEW_CHECKLIST.md) — package and integration review.
