# search-result-projector

Turn verified source locations into compact, ordered evidence for the client.

## Current source

[lib.rs](src/lib.rs) implements `project_candidate_set` and `verify_handle_target_binding`.
The first accepts source-backed candidates plus separately supplied metadata and already minted handles.
It sorts by raw score, applies candidate/source-byte limits and emits the older handle-oriented
`SearchCandidateSet`; it neither mints handles nor emits excerpts. Coverage is supplied by its caller.
The binding helper compares source membership, revision and unit IDs; it does not resolve a handle.

Current daemon uses appear in
[indexed_query_process tests](../../../bins/eliot-searchd/tests/indexed_query_process.rs).
There is no connected public indexed-result projector demonstrated by those calls.

## Implementation direction

Follow [query implementation](../../../docs/implementation/README.md#4-simplify-and-connect-the-query)
and [FUNCTIONS.md](FUNCTIONS.md). Produce an ordered list of source/revision, location/range, short
excerpt and useful freshness/gaps, with an optional next-page reference.

Accept the final ranked order. Derive public evidence fields from the same verified source object;
remove duplicate metadata, mandatory receipts and traces that have no consumer. Enforce the encoded
response limit as well as readback limits. Use the
[single temporary window](../search-continuation/README.md#one-temporary-owner) for paging; projection
is a direct transformation, not another result lifecycle.
