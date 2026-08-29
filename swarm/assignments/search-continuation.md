# `search-continuation` implementation packet

**Path:** `crates/search-query/search-continuation`  
**Capability:** C27  
**Delivery:** W4 / P08; hardening W7 / P13  
**Gate:** BLOCKED until planner, access, pins, contracts and continuation ports are accepted  
**Trace:** S14.3-S14.4, S26, H12, P08, P13

## Mission

Issue opaque continuation tokens while owning a separate tagged server record for either a bounded
in-memory window/pin or an explicit durable replan checkpoint.

## Owns

Token issuance/digest lookup, TTL/count/binding state, candidate-window/pin retention, durable job
checkpoint records, live fence revalidation, issued-candidate suppression, expiry and invalidation.

## Must not own

Public binding/plan/fence/cursor fields, raw Qdrant offsets/scores/point IDs in tokens, indefinite pins,
silent refresh against a newer corpus, durable records for ordinary reads, or durable unsaved bytes.

## Logical operations

1. `create_ephemeral(plan, window, pin, binding, policy) -> Result<ContinuationHandle, ContinuationError>`
2. `create_durable_replan_checkpoint(job, plan, binding, policy) -> Result<ContinuationHandle, ContinuationError>`
3. `resolve_token(handle) -> Result<ContinuationRecord, ContinuationError>`
4. `expand(handle, current_state, budget) -> Result<ContinuationExpansion, ContinuationError>`
5. `expire(now) -> ExpiryReceipt`
6. `invalidate(change) -> InvalidationReceipt`

## Invariants

- public token exposes no binding, durability, plan fingerprint, route/security fence or window ref;
- token digest, not plaintext token, is stored;
- every expansion reauthorizes binding and live security/view/route state;
- ephemeral variant is memory-only, restart-invalid and pin-bounded;
- durable variant belongs to an explicit durable job, replans under stored fence and owns no
  process-local pin or unsaved bytes;
- expired fence returns `SNAPSHOT_EXPIRED` rather than silently refreshing.

## Exit evidence

- wire token contains no cursor/binding/plan/fence;
- token entropy/redaction fixture;
- binding and live-fence revalidation;
- restart invalidates ephemeral window and releases pin;
- durable checkpoint rejected for ordinary query/unsaved target;
- issued-candidate suppression remains internal;
- expired/revoked tokens cannot resume;
- bounded pin/window/count tests.

Target `src/` ≤6,000 lines; split review before 8,500 total; hard stop at 10,000 including local tests.
