# search-contracts

**C00 — Versioned contracts.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Define the complete vendor-neutral wire and domain contract surface used by every other package.

## Owns

- newtypes and identifiers
- recipes and reason codes
- source/view/membership/residency schemas
- grants, plans, budgets and candidate/result schemas
- anchors, handles, protocol envelopes and capability descriptors

## Must not own

- runtime state or I/O
- redb, Qdrant, Windows or client-vendor types
- implicit string/UUID substitution at domain boundaries
- silently ignored security, scope or budget fields

- **Delivery wave:** W0 / P00
- **Soft source-line target:** 8,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
