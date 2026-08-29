# Agent contract — search-handles

You own only `crates/search-query/search-handles/`. Do not edit another package, the root workspace,
shared contracts or architecture. Missing fields use the contract-change process.

The Architecture 8.4 master is not required for ordinary work. This is the package slice.
Traceability only: S23.2-S23.3, S26, S28.1, H4, H14-H16, P08, P13.

## Mission

Give `expand_handle@1` one state owner: mint opaque handles, enforce TTL/quota/binding, reauthorize
every expansion and revoke affected state.

## Ownership

- cryptographically opaque handle IDs and collision policy
- ephemeral in-memory handle table
- durable source-handle records only for retained immutable revisions
- principal/binding/grant/view/security-generation association
- expansion authorization, disclosure ceilings and range validation
- TTL/count quotas, invalidation and expiry receipts

## Forbidden ownership

- treating possession of a handle as authorization
- ranking, result projection or continuation cursor state
- embedding raw source bytes, paths, Qdrant IDs or vendor cursors in public tokens
- durable handles to unsaved editor bytes or unretained revisions
- indefinite TTL or silent expansion against a newer source view
- opening redb/CAS/Qdrant directly

## Allowed dependencies

`search-contracts`, `search-domain`. Current authorization, revision readback and durable record storage
are injected through vendor-neutral ports. No concrete redb, Qdrant or revision-store dependency.

## Required logical surface

- `HandleStore::mint_ephemeral(subject, binding, limits) -> Result<SourceHandle, HandleError>`
- `HandleStore::mint_durable(subject, retention_receipt, binding, limits) -> Result<SourceHandle, HandleError>`
- `HandleStore::expand(handle, request, live_state, ports) -> Result<HandleExpansion, HandleError>`
- `HandleStore::invalidate(scope) -> InvalidationReceipt`
- `HandleStore::expire(now) -> ExpiryReceipt`
- `HandleStore::revalidate(handle, live_state) -> Result<HandlePermit, HandleError>`

## Failure surface

Relevant reasons include `HANDLE_NOT_FOUND`, `HANDLE_EXPIRED`, `HANDLE_BINDING_MISMATCH`,
`ACCESS_REVOKED`, `PURGED`, `SNAPSHOT_EXPIRED`, `DISCLOSURE_CEILING_EXCEEDED` and
`DURABLE_HANDLE_INELIGIBLE`.

## Test seams and exit evidence

- `handle possession never bypasses current grant validation`
- `ephemeral handle expires and releases all associated state`
- `durable handle requires immutable retained SourceRevision`
- `unsaved bytes can never enter durable handle state`
- `revocation purge owner-epoch or view drift invalidates expansion`
- `token reveals no path content point ID score or cursor`
- `range/disclosure budgets are enforced before source readback`

## Size and split guard

- Delivery wave: **W4 / P08; durable/security hardening W7 / P13**
- Soft `src/` target: **6,500 lines**
- Hard review threshold: **10,000 hand-written Rust lines**
- Continuation state remains in `search-continuation`; response assembly remains in
  `search-result-projector`.

## Definition of done

The recipe has one owner, all expansions reauthorize against live state, durable eligibility is proven,
and tokens are bounded opaque references rather than serialized source/query state.
