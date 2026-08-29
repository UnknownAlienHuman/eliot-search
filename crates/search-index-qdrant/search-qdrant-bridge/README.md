# search-qdrant-bridge

**C15 — Qdrant data-plane bridge.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Own qualified Qdrant collection, point and query operations behind vendor-neutral Search ports.

## Owns

- capability and collection-schema probes
- strict-mode indexes
- exact point mutation/readback/delete transport
- filtered query/count operations
- private vendor-type translation

## Must not own

- executable/process/ACL/Job Object lifecycle
- secret storage
- recipe, access, publication or result semantics
- vendor types in public ports

- **Delivery wave:** W3 / P05
- **Soft source-line target:** 7,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
