# search-result-projector — implemented functions

## Current API

In [lib.rs](src/lib.rs):

| Entry point | Actual behavior |
|---|---|
| `project_candidate_set(envelope, inputs, budget)` | Consumes `Vec<CandidateProjectionInput>`, sorts by raw score, checks supplied source/emission metadata, applies candidate/source-byte limits and returns `ProjectedCandidateSet { result, omissions }`. |
| `verify_handle_target_binding(candidate, expected_source_membership, expected_source_revision, expected_unit)` | Compares those three expected IDs with the verified source. No handle token or store is an input. |

`CandidateProjectionInput` combines `SourceBackedCandidate` with `CandidatePublicMetadata`, including
an already minted `SearchSourceHandle`. `ProjectionBudget` bounds candidate count and validated source
bytes. There is no recipe dispatcher, handle factory, source reader or serialized-response budget here.

## Corrections for integration

- Preserve upstream order. Current raw-score sorting reverses a valid fused order when, for example,
  its first result has raw score 0.3 in one population and the second has 100 in another. The input has
  no fused-rank field. Carry the ranked sequence directly; do not recover rank from unrelated scores.
- Select and validate a bounded page incrementally. Current inputs already own verified bytes and
  minted handles before projection can omit them. Avoid reading/minting every result just to discard
  most of them; do not leave omitted handles in a separate catalog. Bound neighbor/relationship work
  before loading it; a small final token budget cannot limit an earlier full graph scan.
- Bound actual output and gap detail. Source-byte totals do not measure encoded metadata, and
  `omissions` grows once per rejected input. Return bounded gap samples/counts and reflect projection
  truncation in the response; the current function copies `envelope.coverage` unchanged.
- Build identity, location and excerpt from one verified source reference. Separately supplied
  `CandidatePublicMetadata` is not verified by a handle lookup; the binding helper cannot establish
  that its public handle resolves to the candidate.

The [query implementation](../../../docs/implementation/README.md#turn-a-point-into-evidence) owns
manifest lookup and actual-byte verification. Group the selected ranges by revision/representation,
merge same-basis overlaps before I/O, and distribute verified excerpts without losing original locators
or rank. Reuse those excerpts in the same bounded query window. Current public expansion's
[`read_revision_range`](../../../bins/eliot-searchd/src/secure_direct_store/kernel/read.rs) reads the
whole revision and copies a slice; repeated tiny expansions repeat that cost. Change its actual reader
with the validator handoff. Whole-object integrity may require one streamed verification pass; a range
request does not itself establish bounded total read bytes. Pass the request limits/deadline to the
reader and count scanned bytes even when only a few ranges are retained. Report unfinished verification
without imposing a new rehash-per-excerpt policy. The encoder and CLI change with the compact response;
neither a separate context-builder service nor the old envelope is a prerequisite.
