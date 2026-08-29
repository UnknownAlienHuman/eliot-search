# search-control-redb

**C02 — Bounded redb control journal.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Persist only bounded technical control state and publish immutable snapshots for read-only hot paths.

## Owns

- journal schema and migrations
- publication intents/receipts and route metadata
- source/control references, cursors and fences
- atomic Arc<ControlSnapshot> publication
- corruption quarantine and write counters

## Must not own

- source bodies or extracted text
- postings, vectors or term statistics
- ranked candidate/query history storage
- reverse-engineering currentness from orphaned Qdrant data

- **Delivery wave:** W1 / P02
- **Soft source-line target:** 8,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
