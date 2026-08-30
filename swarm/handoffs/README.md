# Accepted package handoffs

This directory is reserved for integration-owned, append-only accepted package/API records.

Canonical layout:

```text
swarm/handoffs/<package>/<handoff_id>.toml
```

`handoff_id` is the unique append-only record-path identity. `api_schema_digest` is the accepted public-
surface identity and may legitimately remain unchanged across different reviewed implementations or
evidence revisions; it must not be used as the filename.

A handoff binds the exact submission and independent review, base and final commits, complete changed-file
digest, public API/configuration/fixture/error identities, accepted dependency handoffs, evidence,
compatibility and any required consumer actions. Package writers do not create or edit accepted records.

Rules:

- downstream packages consume exact accepted commits, handoff records and API digests;
- branch heads and worktrees are never handoffs;
- accepted records are immutable;
- a correction writes a new record and append-only supersession receipt; the old record is never edited;
- rejected and incomplete work does not enter this directory;
- a package handoff cannot claim gate or wave acceptance;
- record content must not contain source bodies, secrets or raw query text.

The concrete record schema is fixed by `swarm/schemas/package-handoff-v1.toml`,
`swarm/orchestration.toml`, `swarm/PACKAGE_HANDOFF_TEMPLATE.md` and `swarm/API_HANDOFF_TEMPLATE.md`.
