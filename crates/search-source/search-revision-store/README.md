# search-revision-store

**C07 — Immutable revision CAS.**

**Status:** the pure vendor-neutral revision state machine is implemented. The package also owns the bounded no-clobber filesystem lifecycle for legacy DIRECT revision objects during T02 migration. Canonical residency-aware CAS integration, encryption/secret adapters, lifecycle inventory, and restore flows remain incomplete and must not be inferred from that compatibility adapter.

Admit, retain and reopen immutable source revisions under complete residency identities.

## Owns

- residency-key-derived CAS identities
- immutable revision admission and exact readback contracts
- bounded legacy `.bin` / `.dpapi` exact reads and no-clobber publication
- stable native-identity and locator fencing through an injected platform
- replay fencing, unknown-outcome recovery, quarantine, and purge tombstones
- retention/lifecycle enforcement surfaces
- copy/re-encrypt transition contracts

## Must not own

- query language or ranking
- global content-digest-only CAS namespace
- cross-domain co-residency, ciphertext or key reuse
- source identity or access authorization
- DPAPI/keyring/secret acquisition or plaintext verification policy
- preparation/materialization artifact storage
- unqualified filesystem behavior hidden inside the pure kernel

- **Delivery wave:** W2 / P04
- **Soft source-line target:** 8,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
