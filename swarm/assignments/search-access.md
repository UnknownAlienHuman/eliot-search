# `search-access` implementation packet

**Path:** `crates/search-query/search-access`  
**Capability:** C18  
**Delivery:** W4 / P08; hardening W7 / P13  
**Gate:** BLOCKED until W3 route/publication receipts and protocol grant contracts are accepted  
**Trace:** S14.4, S19, H14, P08, P13  
**Direct public handoffs:** `search-contracts`, `search-domain`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Validate bounded read grants, intersect requested scope with server authority, compile safe retrieval legs and enforce live restrictive revocation.

## Owns

- grant validation and binding checks
- server-authoritative membership/scope intersection
- access/scoring partition and safe-leg compilation
- SecurityMutationBarrier and immutable LiveDenySnapshot semantics

## Must not own

- client authority or policy invention
- raw Qdrant filters/collections/point IDs from clients
- post-filter-only security
- durable lease writes for ordinary reads

## Logical primitives

- ValidatedGrant, GrantValidationContext, ScopeIntersection, SafeRetrievalLeg, OverlapFreeRouteProof, SecurityMutation, LiveDenySnapshot, AccessCheckpoint

## Logical operations

1. `validate_grant(claims, binding, now) -> Result<ValidatedGrant, AccessError>`
2. `intersect_scope(request, grant, registry) -> Result<ScopeIntersection, AccessError>`
3. `compile_safe_legs(scope, routes, proofs) -> Result<Vec<SafeRetrievalLeg>, AccessError>`
4. `apply_security_mutation(command) -> Result<SecurityMutationReceipt, AccessError>`
5. `recheck_live_access(context, checkpoint) -> Result<AccessDecision, AccessError>`
6. `classify_contaminated_legs(execution, new_snapshot) -> Vec<LegId>`

## Required invariants

- access/currentness filter applies before candidates, IDF, counts and traces
- equivalent memberships share an IDF leg only with current overlap-free proof
- restrictive mutation acknowledgement follows durable generation, live snapshot publication and invalidations
- a leg influenced by revoked population is discarded/replanned whole
- grant never widens server-authoritative membership

## Typed failure surface

- `ACCESS_REVOKED`
- `GRANT_INVALID`
- `GRANT_EXPIRED`
- `SCOPE_NOT_AUTHORIZED`
- `SECURITY_FAIL_CLOSED`
- `OVERLAP_PROOF_MISSING`

## Exit tests / evidence

- `access_noninterference_when_inaccessible_corpus_changes`
- `raw_vendor_scope_rejected`
- `revocation_at_every_checkpoint`
- `contaminated_leg_discarded_not_sanitized`
- `security_snapshot_publish_failure_closes_domain`
- `hot_grant_validation_no_redb_write`

## Suggested internal modules

```text
search-access/src/
  grant.rs
  scope.rs
  partition.rs
  leg.rs
  overlap.rs
  barrier.rs
  deny.rs
  checkpoint.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 7,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Grant validation and live deny stay together while one security linearization path governs them; split only after a separately audited boundary.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.
