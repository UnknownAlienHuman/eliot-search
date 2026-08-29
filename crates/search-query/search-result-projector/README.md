# search-result-projector

**C26 — Result Projector.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Project validated candidates into bounded evidence-oriented responses and request opaque handles from `search-handles`.

## Owns

- SearchCandidateSet/card assembly
- coverage, freshness and gap semantics
- bounded ranking trace and response budgets
- deterministic handle-subject selection

## Must not own

- raw full files, vendor details or client admission
- handle storage, expansion or authorization

- **Delivery wave:** W4 / P08
- **Soft source-line target:** 7,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
