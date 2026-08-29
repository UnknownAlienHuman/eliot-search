# P18 advanced-scale supplement — `search-qdrant-bridge`

**Status:** blocked until accepted P15, measured one-shard bottleneck, dedicated ADR and exact topology
qualification. Existing `FUNCTIONS.md` remains authoritative for ordinary data-plane operations.

## Owns

- exact server/client/topology capability probe for one selected scale profile;
- candidate collection topology/schema creation and readback;
- bounded query/mutation behavior under the selected topology;
- scoring, filtered-IDF, strict-mode and response-shape equivalence evidence.

## Required operations

```text
validate_scale_profile(profile, artifact_receipts) -> QualifiedScaleProfile
probe_scale_capabilities(disposable_route, profile, context) -> ScaleCapabilityReceipt
create_scale_candidate_collection(profile, schema, context) -> CandidateCollectionReceipt
verify_scale_candidate_schema(route, expected, context) -> SchemaReceipt
validate_scale_query_equivalence(baseline, candidate, fixtures, context) -> EquivalenceReceipt
validate_scale_mutation_readback(route, fixtures, context) -> MutationEquivalenceReceipt
```

## Invariants

- no topology selected in scaffold;
- active collection schema/topology is never mutated in place;
- exact payload/vector/profile/strict indexes exist before ingest;
- access/currentness/shadow/purge and filtered-IDF predicates remain equivalent;
- query fanout, pages, memory and queues are finite;
- different scoring/IDF semantics require a distinct accepted scoring/product profile;
- bridge never commits serving route or owns migration recovery.

## Required evidence

Exact Windows artifacts and profile; disposable topology fixture; signed epoch/open upper bound; strict
unindexed rejection; filtered-IDF noninterference; exact mutation/readback/count; fanout/resource bounds;
baseline/candidate scoring fixtures; process restart and partial-shard failure; vendor types remain
private.
