# Function contract — `search-handles`

**Status:** W4/P08 handle contract; durable/purge hardening continues in W7/P13.

Public handles are opaque CSPRNG bearer locators. Detailed source, binding, plan, view, anchor,
residency and authorization state exists only in server-owned records keyed by token digest.

## Minting

### `mint_ephemeral(target, binding, policy, randomness) -> Result<SearchSourceHandle, HandleError>`

Atomically creates a memory-only record and returns one non-self-describing token. Unsaved targets must
bind the authenticated buffer snapshot/session and cannot outlive it. Minting is intentionally
non-idempotent; callers do not retry blindly after losing a returned token.

### `mint_durable_source(target, retention_lease, binding, policy, ports, randomness) -> Result<SearchSourceHandle, HandleError>`

Requires an immutable retained `SourceRevision`, native anchor, exact excerpt digest, current residency
permission and durable-handle quota. Unsaved/current-path targets are structurally ineligible.

### `token_digest(handle) -> HandleTokenDigest`

Uses a dedicated domain and never logs or serializes plaintext token bytes. Server records and receipts
store only the digest/reference.

## Resolution and expansion

### `resolve(handle, store) -> Result<SearchSourceHandleRecord, HandleError>`

Performs constant-behavior token-digest lookup where practical and returns no existence detail beyond
the binding-safe failure contract.

### `revalidate(record, request, live_state) -> Result<HandlePermit, HandleError>`

Checks binding/principal, current grant/revocation, owner generation, source/workspace view, residency,
retention lease, purge/tombstone, buffer snapshot, TTL and disclosure/range ceilings. Possession grants
no authority.

### `expand(handle, request, live_state, ports, context) -> Result<HandleExpansion, HandleError>`

Validates budgets before readback, reopens the exact revision/anchor, verifies digest/assurance and
rechecks live state immediately before returning bounded bytes/metadata.

## Cancellation and deadlines

- Minting checks cancellation/deadline before randomness consumption and before record insertion.
- If cancellation or deadline occurs after record insertion but before token delivery, the exact record
  is invalidated and no successful token is reported; uncertain insertion is resolved by the package's
  operation identity/record lookup rather than blind reminting.
- Expansion checks cancellation/deadline before source readback, between bounded read/range operations
  and immediately before emission.
- Cancellation after bytes were read but before emission returns no bytes and no reusable permit.
- A deadline never bypasses the final live binding/grant/owner/view/residency/purge recheck.
- Invalidation and expiry are monotonic; cancellation may stop between bounded batches, returning a
  resumable content-free receipt without resurrecting already invalidated records.

## Lifecycle

### `invalidate(scope, generation, store) -> InvalidationReceipt`

Monotonic and idempotent for owner/view/security/purge/buffer/retention changes. Invalidated records
cannot be resurrected by token replay or restore.

### `expire(now, policy, store) -> ExpiryReceipt`

Expires bounded batches. Caller supplies time. Restart invalidates all ephemeral records and releases
associated memory/pins.

### `apply_live_limits(old, new, store) -> Result<ConfigApplyReceipt, HandleError>`

Implements restrictive changes from `config/sections/handles.md`; excess/over-age records are explicitly
invalidated. `durable_unsaved_allowed=true` is always rejected.

## Required fixtures

Token entropy/opacity; plaintext absent from logs/debug/receipts; provider result cannot serialize
server record; possession without authorization denied; ephemeral restart/buffer-close invalidation;
durable retained-revision requirement; owner/view/residency/purge drift; range budget before readback;
cancellation/deadline before readback and immediately before emission; uncertain mint cleanup/recovery;
quota/TTL reduction receipts; fake persistence/readback ports.
