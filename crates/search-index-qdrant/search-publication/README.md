# search-publication

**C16 — Publication Coordinator.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Own the serialized epoch publication/commit/recovery state machine through vendor-neutral journal and index ports.

## Owns

- publication intents, exact mutation sequence and readback
- guarded VisibleEpoch commit
- compensation/recovery/doctor decisions
- committed retired-point manifests

## Must not own

- concrete redb/Qdrant clients
- broad-filter closure or physical reclaim
- query interpretation or source truth

- **Delivery wave:** W3 / P07
- **Soft source-line target:** 7,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
