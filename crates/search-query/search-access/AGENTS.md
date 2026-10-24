# Working on search-access

Read the [architecture](../../../docs/architecture/README.md),
[query design](../README.md) and [source map](FUNCTIONS.md).

Reduce this package to selected-scope resolution or remove it after moving that logic to query/view
composition. The baseline local input is a requested source subset plus one pinned `PublishedView`.
Return one compatible population for retrieval, IDF, exact inventory and evidence validation.

Remove local signed grants, recipe permissions, snapshot/predicate digests, overlap proofs, per-membership
legs, security checkpoints and durable restriction barriers with their callers. Do not replace
`LiveExpansionBarrier::clean()` with a larger permit system; delete the obsolete handle/continuation path.

A selected restricted-client adapter may authenticate one session, derive an allowed source set and bind
its query windows. Revocation closes/cancels that session. Keep this concrete policy separate from the
owning-account path and do not enumerate all internal objects into a durable invalidation journal.

Source changes publish a new view. Existing local queries use their pinned immutable view; new requests see
the new one. Verify no widening, compatible multi-membership retrieval/IDF, exact inventory parity,
restricted-session cancellation and excluded data not affecting candidates or statistics.
