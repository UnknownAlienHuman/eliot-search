# search-result-projector

**C26 — Result Projector.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Project validated candidates and proof/comparison products into bounded evidence-oriented responses.
Opaque handle state is owned by `search-handles`.

## Owns

- SearchCandidateSet assembly
- coverage, freshness and gap semantics
- bounded ranking trace and response budgets
- deterministic selection of handle subjects

## Must not own

- raw full files or unbounded chunks
- Qdrant/vendor details
- belief/admission decisions
- handle storage, expansion or authorization

- **Delivery wave:** W4 / P08
- **Soft source-line target:** 7,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
