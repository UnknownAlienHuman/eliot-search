# search-runtime-owner

**C01 — Data-root runtime owner.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Guarantee that exactly one process incarnation owns one data root and expose a fenced lifecycle to the daemon.

## Owns

- data-root lease and owner epoch
- standalone/managed mode fence
- crash/reopen ownership recovery
- clean shutdown and drain state

## Must not own

- retrieval semantics
- Qdrant schema or query operations
- source catalog or access policy

- **Delivery wave:** W1 / P01
- **Soft source-line target:** 4,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
