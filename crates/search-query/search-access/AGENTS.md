# Working on search-access

Read the [query owner](../README.md), [implementation spine](../../../docs/implementation/README.md) and local
[README](README.md). Reduce this package to selected-scope resolution or move the few pure functions into query
composition.

Input is one pinned `PublishedView`, requested/default source set and optional real restricted-session allowed set.
Return:

```text
ordered selected memberships
base population filter = selected sources ∩ session/access restriction
candidate filter = base population + request-only narrowing
exact retained inventory refs
```

The base filter controls both candidates and `SearchParams.idf.corpus`. Request-only path/kind/topic filters affect
candidates only. If base equals the whole collection, omit filter/IDF. Explicit empty never widens; unknown,
foreign or incomplete membership fails or reports a gap.

Different physical collections/retrieval profiles remain different populations. Do not compare raw scores across
them or create per-source legs. Qdrant BM25 statistics are shard-local; do not introduce a custom-shard portfolio.

Remove signed grants, recipe permissions, snapshot/predicate digests, overlap proofs, eligibility plans,
per-membership legs, phase checkpoints and durable restriction barriers with callers. A selected restricted-client
profile authenticates one session, derives one allowed source set and cancels/removes that session's windows on
revocation—no durable influence journal.

Source changes publish a new view. Existing local requests use their pinned view; new requests see new state.
After code verify default/explicit/empty/unknown sets, restricted narrowing, base filter parity between candidates
and IDF, request-only filter separation, exact/evidence inventory parity and old-view behavior. R051 changes docs
only.
