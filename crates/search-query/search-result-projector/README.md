# search-result-projector

**C26 — Compact result projection.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Project validated candidates, comparison and exact reports into bounded evidence-oriented responses and handles.

## Owns

- SearchCandidateSet assembly
- coverage/freshness/gap semantics
- default 2-4 recommended handles
- bounded non-content ranking trace
- result byte and disclosure budgets

## Must not own

- raw full files or unbounded chunk arrays
- Qdrant collections, filters, offsets or payload exposure
- belief/admission/finish dispositions
- calling top-k coverage complete_scope

- **Delivery wave:** W4 / P08
- **Soft source-line target:** 7,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
