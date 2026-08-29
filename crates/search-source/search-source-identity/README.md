# search-source-identity

**C04 — Source identity and namespace ownership.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Derive stable source identity, retain path history and enforce single-writer namespace ownership and cutover.

## Owns

- SourceIdentity derivation
- PathBinding history
- revision occurrence identity hooks
- SourceNamespaceOwnership state machine
- cutover receipt validation and fencing

## Must not own

- corpus/access policy inside SourceIdentity
- file content reads
- retrieval membership or ranking

- **Delivery wave:** W2 / P03
- **Soft source-line target:** 6,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
