# search-revision-store

**C07 — Immutable revision CAS.**

**Status:** the pure vendor-neutral revision state machine is implemented. During T02 migration the package also owns the bounded no-clobber filesystem lifecycle, closed layout/name grammar, deterministic physical inventory model, checkpoint/cursor/page rules and exact report projection for legacy DIRECT revision objects. Canonical residency-aware CAS integration, encryption/secret adapters, lifecycle inventory, and restore flows remain incomplete and must not be inferred from that compatibility adapter.

Admit, retain and reopen immutable source revisions under complete residency identities.

## Owns

- residency-key-derived CAS identities
- immutable revision admission and exact readback contracts
- bounded legacy `.bin` / `.dpapi` exact reads and no-clobber publication
- stable native-identity and locator fencing through an injected platform
- legacy `revisions/<shard>/<id>.<encoding>` layout and closed inventory grammar
- deterministic names/sizes/mtimes inventory digest and stable counts
- orphan checkpoint, canonical cursor, bounded page selection and exact JSON report bytes
- stable referenced/orphan/temporary physical classification tags
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
- data-root traversal, metadata acquisition or object-content hashing
- unqualified filesystem behavior hidden inside the pure kernel

- **Delivery wave:** W2 / P04
- **Soft source-line target:** 8,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
