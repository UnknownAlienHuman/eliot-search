# search-domain

**shared pure kernel — Pure invariant algebra.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Implement pure state transitions and deterministic decision rules over search-contracts types without owning any external capability.

## Owns

- pure validation and transition functions
- canonical ordering and plan-fingerprint rules
- eligibility/filter AST semantics
- coverage classification and invariant proofs

## Must not own

- I/O, clocks, process handles or vendor clients
- becoming a dumping ground for capability-specific logic
- owning source, query, publication or access state

- **Delivery wave:** W0 / P00
- **Soft source-line target:** 7,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
