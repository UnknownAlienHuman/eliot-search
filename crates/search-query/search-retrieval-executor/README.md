# search-retrieval-executor

Execute bounded retrieval legs and combine their nominations before source validation.

## Current source

[lib.rs](src/lib.rs) implements admission checks, leg tickets, pin acquisition, dispatch, normalization
and weighted rank fusion. Admission checks supplied queue counts; it does not reserve a queue slot or
implement a scheduler. [indexed.rs](src/indexed.rs) implements lexical retrieval through
`IndexedRetrievalPort`, currently including an unconditional exact count.

The daemon's [execute_pinned_leg](../../../bins/eliot-searchd/src/query_composition.rs) still takes
`ProcessTestIndexedPort`; its current callers are process tests. These APIs do not establish a connected
public indexed query. [FUNCTIONS.md](FUNCTIONS.md) names their actual behavior and integration defects.

## Implementation direction

Follow [query implementation](../../../docs/implementation/README.md#4-simplify-and-connect-the-query).
Use one request operation with its actual scope, published-view pins, deadline and cancellation state.
Keep pure ranking functions; connect the existing real indexed port and remove replaced dispatch paths.

Deduplicate by the resolved published source/unit identity and scope/revision, then preserve the fused
order through validation and projection. A leg identifies a ranking contribution, not a separate result.
Paging uses the [single temporary window](../search-continuation/README.md#one-temporary-owner); it does
not create another executor or durable query history.
