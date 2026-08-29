# search-revision-store

**C07 — Immutable revision CAS.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Admit, retain and reopen immutable source revisions under complete residency identities.

## Owns

- residency-key-derived CAS paths
- atomic temp/fsync/rename writes
- raw revision and manifest integrity
- retention leases and exact reopen
- copy/re-encrypt transition receipts

## Must not own

- query language or ranking
- global content-digest-only CAS namespace
- cross-domain co-residency, ciphertext or key reuse
- source identity or access authorization

- **Delivery wave:** W2 / P04
- **Soft source-line target:** 8,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
