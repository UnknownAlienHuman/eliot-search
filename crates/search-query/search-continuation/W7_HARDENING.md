# W7 hardening — `search-continuation`

This packet refines security/lifecycle behavior for ephemeral candidate windows and durable replan
checkpoints. The W4 `FUNCTIONS.md` remains authoritative for creation/expansion basics.

## `invalidate_lifecycle_scope`

```text
invalidate_lifecycle_scope(set, live_fence, store, pins, operation)
    -> Result<ContinuationInvalidationReceipt, ContinuationError>
```

Invalidates exact records by binding/grant, owner/source/workspace view, route/epoch/profile, purge,
job and dependency API generation. Ephemeral candidate windows and pins are released atomically with
record invalidation where possible; uncertainty remains fail-closed.

Same operation/equal set is idempotent. Receipt stores token digests and bounded counts only.

## Expansion checkpoints

Every expansion/replan checks live security and record fences:

1. before lookup response shaping;
2. before pin renewal or planner/executor invocation;
3. after retrieval/validation completes;
4. before next token/window emission.

A restrictive change discards the new window and releases/invalidates pins. Existing issued results are
not retracted by this package but their source handles reauthorize independently.

## Durable checkpoint eligibility

Durable checkpoint requires an explicit governed job, immutable admitted source scope, finite retention,
accepted dependency/API/profile identities and issued-candidate fingerprints. It contains no raw Qdrant
cursor/score/point ID, process-local pin, source body or unsaved snapshot.

A changed dependency/API/profile/source view returns `SNAPSHOT_EXPIRED` or explicit replan-required gap.
It never transparently continues against a newer corpus while preserving the old continuation identity.

## Purge and restore

Purge invalidation permanently blocks affected continuation records/token digests under the tombstone
generation. Ephemeral records never restore. Durable checkpoints are imported only into quarantine,
then current authorization, source/owner/view, dependencies, issued fingerprints and purge tombstones
are revalidated. Baseline should issue a fresh token after replan/admission.

## Required tests

- revocation/purge/route/profile drift at every expansion checkpoint;
- invalidation releases every process-local pin/window and is idempotent;
- restrictive change after execution but before emission returns no new window/token;
- durable checkpoint contains no unsaved bytes or process pin/vendor cursor;
- dependency/source-view drift cannot silently refresh;
- purge tombstone blocks restore/replan resurrection;
- ephemeral continuation absent after restart/restore;
- restored durable token is not automatically serving-valid;
- invalidation receipt cannot satisfy projection/CAS purge layers.
