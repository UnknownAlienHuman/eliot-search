# search-publication

**C16 — Epoch publication coordinator.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Serialize projection commits, verify exact readback and linearize visibility only through guarded control-journal commit.

## Owns

- publication actor and state machine
- durable intents and receipts orchestration
- exact new/old point mutation sequence
- generation guard CAS commit
- recovery, compensation and doctor command domain

## Must not own

- multiple active commit transactions
- reusing skipped epochs
- Qdrant alias as commit point
- broad payload closure on correctness paths
- staging later epoch while earlier is unresolved
- depending on the concrete redb adapter; durable state is reached through a vendor-neutral port

- **Delivery wave:** W3 / P07
- **Soft source-line target:** 9,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
