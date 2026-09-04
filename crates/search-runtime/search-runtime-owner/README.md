# search-runtime-owner

**C01 — Data-root runtime owner.**

**Status:** pure W1 ownership, fencing, recovery, drain, and release state machine implemented. Concrete OS lock, process observation, durable-record, ACL, and Windows path adapters remain separate runtime work.

Guarantees that exactly one process incarnation can be admitted as owner of one canonical data root and exposes a fenced lifecycle to the daemon.

## Owns

- data-root lease and owner epoch
- standalone/managed mode fence
- crash/reopen ownership recovery decisions
- active/draining/releasing/unknown/quarantined lifecycle
- process-local owner guard, drain token, release permit, and shutdown receipt
- exact operation identity and readback-bound recovery

## Must not own

- filesystem, process, clock, database, registry, or network I/O
- retrieval semantics
- Qdrant schema or query operations
- source catalog or access policy

- **Delivery wave:** W1 / P01
- **Soft source-line target:** 4,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
