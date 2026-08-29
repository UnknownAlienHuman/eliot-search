# search-qdrant-bridge

**C15 — Qdrant supervisor and bridge.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Own all qualified Qdrant process and transport details behind vendor-neutral Search ports.

## Owns

- exact artifact qualification and process identity
- loopback/auth/ACL/Job Object lifecycle
- collection schema and capability probes
- strict-mode indexes
- upsert, close, query, count and exact readback transport

## Must not own

- recipe meaning, access authority or result interpretation
- vendor types in public ports
- automatic upgrades or unpinned latest
- CLI/worker/client direct Qdrant access

- **Delivery wave:** W3 / P05
- **Soft source-line target:** 9,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
