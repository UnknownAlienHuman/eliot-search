# search-result-projector — format verified evidence or disappear

This package is not a ranking, authorization, evidence or handle owner. It may format already ordered,
already verified evidence into bounded public cards. If that transformation remains small, fold it into the
query handler and delete the package.

Read the [query cutover](../README.md), [evidence map](../search-candidate-validator/FUNCTIONS.md) and
[query-window map](../search-continuation/FUNCTIONS.md).

## Verified current source

`src/lib.rs` is 10,337 bytes. `project_candidate_set` receives a complete vector, re-sorts it by raw score,
receives already verified source bytes and an already minted `SearchSourceHandle`, applies limits only after
that work and returns the old handle-oriented result family. `verify_handle_target_binding` compares three
supplied IDs but resolves no handle or evidence.

Repository search finds the projector in its own source, an old swarm assignment and the indexed process
fixture; no normal product query caller was found.

## Target

Input is final-order `VerifiedEvidenceItem` values selected before retained-source I/O:

```text
VerifiedEvidenceItem {
  stable point/evidence identity
  source/revision/representation/location
  verified excerpt
  final rank and optional branch contributions
  useful candidate gap metadata
}
```

Projection may:

- preserve input order exactly;
- enforce remaining item/encoded-byte limits before retaining output;
- truncate excerpts according to the explicit response contract;
- format `EvidenceCard` values;
- sample bounded gap details while reporting aggregate omitted counts;
- attach optional `WindowItemRef(window_id, item_index)` supplied by the query-window owner.

Projection may not:

- compare or sort raw backend scores;
- resolve scope, manifests, handles or retained objects;
- mint tokens or lifecycle records;
- accept caller coverage/proof Booleans as authority;
- manufacture complete-negative coverage from top-k;
- rebuild another result identity or plan digest.

## Removal

Delete `CandidatePublicMetadata`, mandatory `SearchSourceHandle`, handle-binding helpers, raw-score sorting,
old candidate-set envelopes and one-omission-record-per-rejection behavior with their callers. Change the
current protocol/result types directly; do not add a V3 projector family or forwarding adapter.

## Completion

Final Qdrant/rank order reaches output unchanged, output limits are charged before allocation/retention,
source-backed cards contain the actual verified excerpt/location and paging/item locators are owned by the
single `QueryWindow`.
