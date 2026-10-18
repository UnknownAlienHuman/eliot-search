# search-access

Resolve the requested local corpus against application state and keep retrieval within that explicit
scope and published view. Restricted external clients may add real access checks at their boundary.
The ordinary owning-account path does not need signed grants, overlap certificates or a security
checkpoint protocol around every local transformation.

The [architecture](../../../docs/architecture/README.md) and
[implementation](../../../docs/implementation/README.md) supersede the historical grant/partition
contract. [FUNCTIONS.md](FUNCTIONS.md) records actual calls and the concrete replacement.

## Scope without per-file query fanout

The current compiler creates one leg per source membership unless an `OverlapFreeRouteProof`
allows grouping. The indexed executor then rejects grouped legs containing multiple eligibility plans.
This is an integration defect in source, not a required population model.

Use one eligibility predicate for a compatible selected scope and its pinned published view. The
bridge already supports a membership union and explicitly sends the corpus to Qdrant's native IDF.
Split queries only for different physical routes or search settings that cannot share a request.

Keep unknown/empty-scope handling, actual restrictions and bounded work. Remove proof-required
grouping, local predicate digests and redundant grant machinery from the owning-account query path.
Do not duplicate indexed unit occurrences to represent equivalent memberships; result grouping does
not remove duplicates from the backend's IDF population.
