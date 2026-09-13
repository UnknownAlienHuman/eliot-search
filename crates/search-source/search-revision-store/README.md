# search-revision-store

**C07 — Immutable revision CAS.**

**Status:** the pure vendor-neutral revision state machine is implemented. Concrete atomic object I/O, encryption/secret adapters, exact reopen, lifecycle inventory, and restore flows remain incomplete and must not be inferred from the in-memory kernel.

Admit, retain and reopen immutable source revisions under complete residency identities.

## Owns

- residency-key-derived CAS identities
- immutable revision admission and exact readback contracts
- replay fencing, unknown-outcome recovery, quarantine, and purge tombstones
- retention/lifecycle enforcement surfaces
- copy/re-encrypt transition contracts

## Must not own

- query language or ranking
- global content-digest-only CAS namespace
- cross-domain co-residency, ciphertext or key reuse
- source identity or access authorization
- filesystem, encryption, or secret-store behavior hidden inside the pure kernel

- **Delivery wave:** W2 / P04
- **Soft source-line target:** 8,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
