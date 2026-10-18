# search-result-projector

Turn one already ranked, source-verified page into compact evidence cards. Projection is a pure final
transformation; it does not own retrieval, source I/O, handles, continuations or result lifetime.

## Target input and output

```text
VerifiedEvidenceItem
  final_rank
  source / revision / representation
  logical occurrence and exact range/location
  verified excerpt
  freshness / assurance / role

→ EvidenceCard
  rank, source/location, excerpt, useful gaps
  optional WindowItemRef
```

Accept the executor/fusion order exactly. Apply bounded card/output/gap limits without sorting by raw scores
from incomparable populations. Derive public fields from the same verified item; do not combine a source
candidate with separately minted handle metadata.

Selection and retained-range loading happen before projection in the
[query path](../README.md#target-flow). The one query window owns paging and optional item locators.
[FUNCTIONS.md](FUNCTIONS.md) records current defects and the direct replacement.
