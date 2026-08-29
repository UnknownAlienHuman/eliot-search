# Accepted package handoffs

This directory is reserved for integration-owned, append-only accepted package/API receipts.

Canonical layout:

```text
swarm/handoffs/<package>/<api-schema-digest>.toml
```

A receipt binds package, assignment/ticket, base and final commits, dependency handoff digests,
API/schema digest, evidence refs and reviewer acceptance. Package writers do not create or edit accepted
receipts.

Rules:

- downstream packages consume exact accepted commits and API digests;
- branch heads and worktrees are never handoffs;
- accepted receipts are immutable;
- a correction writes a new digest and marks the old receipt superseded by reference;
- rejected and incomplete work does not enter this directory;
- receipt content must not contain source bodies, secrets or raw query text.

The concrete receipt schema is fixed by `swarm/orchestration.toml`,
`swarm/PACKAGE_HANDOFF_TEMPLATE.md` and `swarm/API_HANDOFF_TEMPLATE.md`.
