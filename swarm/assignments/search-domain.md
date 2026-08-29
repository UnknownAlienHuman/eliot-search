# `search-domain` implementation packet

**Path:** `crates/search-domain`  
**Capability:** shared pure invariant kernel  
**Delivery:** W0 / P00  
**Gate:** CONDITIONAL on accepted `search-contracts` API/schema digest  
**Direct public handoffs:** `search-contracts`

Read `CANONICAL_TYPES.md`, `SOURCE_GRAPH.md`, `QUERY_AND_RESULTS.md`, `REASON_CODES.md` and the accepted
contracts handoff. Do not read contract implementation internals.

## Mission

Implement reusable pure state transitions and deterministic decision rules over contract types.

## Owns

- source-owner and publication transition legality
- eligibility-AST construction/equivalence
- plan-fingerprint input selection and verification
- stable total candidate ordering
- coverage classification and overclaim rejection
- pure invariant predicates with no capability owner

## Must not own

- I/O, clocks, random generation, ports, process handles or vendor clients
- mutable access/source/publication/query state
- capability-specific orchestration or adapter translation

## Required operations

- `transition_source_ownership`
- `transition_publication`
- `build_base_eligibility_predicate`
- `prove_retrieval_idf_filter_equivalence`
- `compute_and_verify_plan_fingerprint`
- `stable_candidate_order`
- `classify_coverage`

## Invariants and tests

- skipped/reverse/dual-owner state transitions fail
- retrieval and IDF base predicates are semantically identical
- equal canonical fingerprint inputs produce equal plan identity
- candidate ordering is total, stable and transitive
- `complete_scope` requires accepted exact proof
- pure code cannot observe wall-clock or mutable external state
- dependency guard permits only `search-contracts`

Target `src/` ≤7,000 lines; split review before 8,500 total; hard stop at 10,000 including local tests.
