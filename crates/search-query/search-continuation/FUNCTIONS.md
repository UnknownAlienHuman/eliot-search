# Function contract — `search-continuation`

**Status:** W4/P08 continuation contract; durable/security hardening continues in W7/P13.

Public continuation values are opaque CSPRNG tokens. Candidate windows, issued-candidate suppression,
plan/view/route/security fences and pins are server-owned records keyed by token digest.

## Creation

### `create_ephemeral(plan, remaining_window, pin_set, binding, policy, randomness) -> Result<ContinuationHandle, ContinuationError>`

Requires a bounded in-memory candidate window and accepted route/epoch pins. The record is
binding-scoped, TTL/count limited and restart-invalid. The token contains no cursor, score, point ID,
plan fingerprint, fence or binding field.

### `create_durable_replan_checkpoint(job, plan, binding, policy, store, randomness) -> Result<ContinuationHandle, ContinuationError>`

Allowed only for an explicit durable job over immutable admitted data. The record stores a replan
checkpoint, accepted dependency/API identities and issued-candidate fingerprints; it owns no
process-local pin and cannot reference unsaved bytes.

### `token_digest(handle) -> ContinuationTokenDigest`

Uses a dedicated domain. Plaintext tokens never appear in persistent records, logs, telemetry or
receipts.

## Expansion

### `resolve(handle, store) -> Result<ContinuationRecord, ContinuationError>`

Performs binding-safe token-digest lookup without disclosing whether a foreign token exists.

### `revalidate(record, binding, live_state, now) -> Result<ContinuationPermit, ContinuationError>`

Checks binding/principal, grant/revocation, owner generation, source/workspace view, route/epoch,
profile, purge, TTL and configured pin/window ceilings. Possession grants no authority.

### `expand_ephemeral(record, permit, budget) -> Result<ContinuationExpansion, ContinuationError>`

Returns the next bounded unissued window and renews a continuation pin only within the original
route/epoch and maximum TTL. It never silently refreshes against newer corpus state.

### `expand_durable(record, permit, planner, executor, budget) -> Result<ContinuationExpansion, ContinuationError>`

Recompiles/re-executes under the stored contract fence and suppresses previously issued stable
candidate fingerprints. A changed/expired dependency returns `SNAPSHOT_EXPIRED` or explicit replan gap,
not a transparent corpus switch.

## Cancellation and deadlines

- Creation checks cancellation/deadline before token generation, pin acquisition and record insertion.
- If cancellation or deadline occurs after pin/record acquisition but before token delivery, the package
  releases the exact pin/window and invalidates the orphan record; uncertain durable insertion is
  resolved by stable operation identity rather than blind reminting.
- Expansion checks cancellation/deadline before pin renewal, before planner/executor dispatch, between
  bounded candidate windows and immediately before terminal emission.
- Cancellation after a window was read or recomputed but before emission returns no candidates and does
  not mark them issued.
- A durable expansion cancelled after dependent work starts preserves the stored replan checkpoint and
  reports truthful incomplete coverage; it cannot switch to a newer fence.
- Deadline expiry never extends TTL or pin lifetime beyond configured maxima and never suppresses a
  `SNAPSHOT_EXPIRED`/security failure.

## Lifecycle

### `invalidate(scope, generation, store) -> InvalidationReceipt`

Monotonic and idempotent for security, purge, owner/view/route/profile and job changes.

### `expire(now, policy, store, pins) -> ExpiryReceipt`

Expires bounded batches and releases all process-local pins/window memory. Caller supplies time.
Restart invalidates every ephemeral record.

### `apply_live_limits(old, new, store, pins) -> Result<ConfigApplyReceipt, ContinuationError>`

Implements restrictive changes in `config/sections/continuations.md`; excess records/pins are explicitly
expired. Durable ordinary queries and durable unsaved targets remain always rejected.

## Required fixtures

Token entropy/opacity/redaction; foreign binding denial; raw Qdrant cursor/score/point ID absent;
restart invalidates ephemeral and releases pin; TTL/count/pin bounds; durable ordinary-query/unsaved
rejection; durable record owns no process pin; stable issued suppression; security/view/route drift;
cancellation/deadline before creation, after pin acquisition and immediately before emission; orphan
record/pin cleanup; expired fence returns `SNAPSHOT_EXPIRED`; no silent refresh.
