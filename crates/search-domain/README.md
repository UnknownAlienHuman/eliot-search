# search-domain

**Shared pure kernel — deterministic product invariant algebra.**

**Status:** substantive pure Rust source exists for assurance, coverage, currentness, eligibility,
visibility, ordering, mutation outcomes, ownership/publication transitions and plan/snapshot fingerprint
verification. The package performs no I/O and does not by itself integrate or qualify a product path.

## Owns

- pure validation and transition functions;
- assurance/currentness/visibility decisions;
- eligibility and retrieval/IDF equivalence semantics;
- coverage classification and complete-negative rules;
- deterministic candidate ordering;
- query-snapshot and plan fingerprint computation/verification;
- mutation outcome/retry classification;
- publication and source-ownership transition checks.

## Must not own

- I/O, clocks, process handles or vendor clients;
- mutable source, query, publication or access state;
- capability-specific adapters;
- client task/admission/finish authority;
- becoming a dumping ground for behavior owned by a concrete package.

This package is `SOURCE`. Its functions become product evidence only through the concrete owners and
executed integration/qualification paths that consume them.

- **Product area:** Architecture S3/S34 and shared semantics
- **Agent instructions:** [AGENTS.md](AGENTS.md)
- **Current status matrix:** [../../docs/product/IMPLEMENTATION_STATUS.md](../../docs/product/IMPLEMENTATION_STATUS.md)
