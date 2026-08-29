# `search-domain` implementation packet

**Path:** `crates/search-domain`  
**Capability:** shared pure invariant kernel  
**Delivery:** W0 / P00  
**Gate:** CONDITIONAL on accepted `search-contracts` digest

Read canonical/type/support/source/query/result/reason projections and the accepted contracts handoff.

## Mission

Implement reusable pure transitions and deterministic decision rules over contract types.

## Owns

- source-owner/publication transition legality;
- query-snapshot and plan fingerprint verification;
- eligibility-AST construction/equivalence;
- stable total candidate ordering;
- coverage classification and overclaim rejection;
- pure drift classification between planned snapshot, current non-security state and emission security.

## Does not own

I/O, clocks, random generation, ports, process handles, vendor clients, mutable capability state or
capability orchestration.

## Required operations

- `transition_source_ownership`
- `transition_publication`
- `compute_and_verify_query_snapshot_fingerprint`
- `compute_and_verify_plan_fingerprint`
- `build_base_eligibility_predicate`
- `prove_retrieval_idf_filter_equivalence`
- `classify_snapshot_drift`
- `stable_candidate_order`
- `classify_coverage`

## Invariants/tests

Illegal/dual-owner/skipped transitions fail; query snapshot includes every S14 axis; generic dependency
cannot replace one; equal canonical inputs give equal fingerprints; restrictive emission changes force
revalidation while planned snapshot is preserved; ordering is total/stable/transitive; complete scope
requires exact proof; pure code observes no external time/state; only contracts dependency.

Target `src/` ≤7,000 lines; split review before 8,500 total; hard stop 10,000 including tests.
