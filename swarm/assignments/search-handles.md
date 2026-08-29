# `search-handles` implementation packet

**Path:** `crates/search-query/search-handles`  
**Capability:** C26/C27 handle support  
**Delivery:** W4 / P08; durable/security hardening W7 / P13  
**Gate:** BLOCKED until source-handle contracts, access checks and revision-retention handoffs are accepted  
**Trace:** S23.2-S23.3, S26, S28.4, H4, H14-H16, P08, P13  
**Direct public handoffs:** `search-contracts`, `search-domain`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust spelling.

## Mission

Give `expand_handle@1` one state owner: mint opaque handles, enforce TTL/quota/binding, reauthorize every expansion and revoke affected state.

## Owns

- cryptographically opaque handle IDs and collision handling
- ephemeral in-memory handle table
- durable source-handle records only for retained immutable revisions
- principal/binding/grant/view/security-generation association
- expansion authorization, disclosure/range budgets and invalidation

## Must not own

- result ranking/projection or continuation candidate windows
- treating possession as authorization
- raw source bytes, absolute paths, Qdrant IDs, scores or cursors in public tokens
- durable handles to unsaved buffers or unretained revisions
- indefinite TTL or silent expansion against a newer source view
- direct redb, CAS or Qdrant access

## Logical primitives

- `HandleId`, `HandleClass`, `HandleRecord`, `HandleBinding`, `HandlePolicy`, `HandlePermit`, `HandleExpansion`, `HandleInvalidation`, `HandleStorePort`

## Logical operations

1. `mint_ephemeral(subject, binding, policy) -> Result<SearchSourceHandle, HandleError>`
2. `mint_durable(subject, retention_receipt, binding, policy) -> Result<SearchSourceHandle, HandleError>`
3. `revalidate(handle, live_state) -> Result<HandlePermit, HandleError>`
4. `expand(handle, request, live_state, ports) -> Result<HandleExpansion, HandleError>`
5. `invalidate(scope) -> InvalidationReceipt`
6. `expire(now) -> ExpiryReceipt`

## Required invariants

- handle possession never bypasses current grant/security validation
- ephemeral handles are memory-only, bounded and restart-invalid
- durable handles require immutable retained `SourceRevision`
- unsaved bytes cannot enter durable handle state
- revocation, purge, owner-generation or source-view drift blocks expansion
- tokens disclose no source content, path, vendor identity, score or cursor

## Typed failure surface

- `HANDLE_NOT_FOUND`
- `HANDLE_EXPIRED`
- `HANDLE_BINDING_MISMATCH`
- `DURABLE_HANDLE_INELIGIBLE`
- `ACCESS_REVOKED`
- `PURGED`
- `DISCLOSURE_CEILING_EXCEEDED`

## Exit tests / evidence

- `handle_possession_never_grants_access`
- `ephemeral_expiry_and_restart_invalidation`
- `durable_requires_retained_immutable_revision`
- `unsaved_never_durable`
- `security_purge_owner_and_view_drift_invalidate`
- `opaque_token_non_disclosure_fixture`
- `range_and_disclosure_budget_before_readback`

## Suggested internal modules

```text
search-handles/src/
  id.rs
  record.rs
  ephemeral.rs
  durable.rs
  permit.rs
  expand.rs
  invalidation.rs
  expiry.rs
  error.rs
```

## Size / split

- Initial `src/` target: **≤ 6,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Continuation state remains in `search-continuation`; response assembly remains in `search-result-projector`.
