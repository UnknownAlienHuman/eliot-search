# `search-handles` implementation packet

**Path:** `crates/search-query/search-handles`  
**Capability:** C26/C27 handle support  
**Delivery:** W4 / P08; durable/security hardening W7 / P13  
**Gate:** BLOCKED until contracts, access, retention and handle ports are accepted  
**Trace:** S23.2-S23.3, S26, S28.4, H4, H14-H16, P08, P13

## Mission

Mint opaque wire handles while owning the separate server-side records, TTL/quota state, live
authorization and invalidation.

## Owns

- CSPRNG token creation, token-digest lookup and collision handling;
- opaque `SearchSourceHandle` issuance;
- memory-only ephemeral records, including authenticated unsaved-buffer targets;
- durable source records only for immutable retained revisions;
- binding/grant/view/security/owner/residency association;
- expansion authorization, disclosure/range budgets, expiry and revocation.

## Must not own

Ranking/projection, continuation windows, self-contained tokens, plaintext token logs, durable unsaved
state, indefinite ungoverned TTL, current-path substitution, or direct redb/CAS/Qdrant access.

## Logical operations

1. `mint_ephemeral(target, binding, policy) -> Result<SearchSourceHandle, HandleError>`
2. `mint_durable_source(target, retention_lease, binding, policy) -> Result<SearchSourceHandle, HandleError>`
3. `resolve_token(handle) -> Result<SearchSourceHandleRecord, HandleError>`
4. `revalidate(record, live_state) -> Result<HandlePermit, HandleError>`
5. `expand(handle, request, live_state, ports) -> Result<HandleExpansion, HandleError>`
6. `invalidate(scope) -> InvalidationReceipt`
7. `expire(now) -> ExpiryReceipt`

## Invariants

- public token exposes no namespace, revision, view, anchor, path, residency or authorization field;
- server stores token digest, never plaintext token;
- handle ID/token possession does not grant access;
- ephemeral unsaved target is memory-only and dies with buffer/session/restart;
- durable target requires immutable retained revision and current retention lease;
- every expansion rechecks grant, binding, owner, view, residency, purge and disclosure budget;
- revoked/expired records are monotonic and cannot be resurrected by token replay.

## Exit evidence

- wire token non-disclosure and minimum entropy fixture;
- token absent from logs/debug/telemetry/receipts;
- server record never serializes as provider result;
- possession without live authorization denied;
- ephemeral restart/buffer-close invalidation;
- durable retained-revision requirement;
- purge/owner/view/residency drift denial;
- range/disclosure budget before readback;
- fake stores prove adapter independence.

Target `src/` ≤6,500 lines; split review before 8,500 total; hard stop at 10,000 including local tests.
