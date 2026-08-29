# Function contract — `search-overlay`

**Status:** W5/P09 logical contract; no overlay runtime implementation exists yet.

The package owns one bounded query-time precedence layer:

```text
published base
  < confirmed saved revision awaiting publication
  < authenticated unsaved editor snapshot
```

Saved and unsaved state have different residency and crash semantics. They share precedence/shadow
logic but never share a persistence shortcut.

## State ownership

The package owns:

- saved-overlay references to admitted immutable revisions awaiting publication;
- process-memory unsaved bytes bound to authenticated editor snapshots;
- per-binding/source overlay revisions and exact shadow sets;
- TTL/quota/invalidation state;
- bounded direct exact/token/structural overlay candidate streams.

It does not own filesystem save, source admission, durable revision creation, Qdrant publication,
source-handle durability or a persistent second index.

## `admit_saved_overlay`

```text
admit_saved_overlay(revision_receipt, membership, base_fence, operation)
    -> Result<SavedOverlayEntry, OverlayError>
```

Requires an admitted immutable `SourceRevision`, compatible residency/access/purge state and exact
source namespace/owner generation. It stores references and bounded structural/lexical preparation
state permitted by the accepted profile; source bytes remain in the revision store.

Same operation identity plus equal receipt is idempotent. A changed receipt under the same operation is
rejected.

## `attach_unsaved_snapshot`

```text
attach_unsaved_snapshot(binding, authenticated_session, snapshot, bytes, limits, now)
    -> Result<UnsavedBufferGuard, OverlayError>
```

**Preconditions**

- IDE/editor session and binding are authenticated and currently authorized;
- snapshot contains exact buffer ID/version/position encoding/source namespace association;
- bytes and per-binding/source quotas are finite;
- caller grants source-read/overlay permission and sensitivity ceiling.

**Postconditions**

- bytes exist only in process-owned guarded memory;
- snapshot is immediately the highest-precedence revision for its exact source/membership;
- base/saved candidates are shadowed before the guard is observable;
- durable state contains only non-reconstructive digest/size/session/invalidation metadata when required;
- no background persistence, provider cache, crash attachment, telemetry payload or evaluation-corpus
  path receives the bytes.

The guard/token is opaque and non-serializable. Possession does not replace current authorization.

## `replace_unsaved_snapshot`

```text
replace_unsaved_snapshot(guard, next_snapshot, next_bytes, limits, now)
    -> Result<OverlayReplacementReceipt, OverlayError>
```

Atomically installs the next version and its shadow fence before retiring the previous version. There is
no interval where stale saved/published candidates become visible. Version regression, buffer identity
change or authorization drift fails closed and invalidates the prior guard as required.

## `close_or_invalidate_unsaved`

```text
close_or_invalidate_unsaved(scope, cause, live_security, now)
    -> OverlayInvalidationReceipt
```

Causes include editor close, replacement, disconnect, TTL, binding revocation, owner-generation change,
purge, quota reduction and daemon shutdown. Invalidation is idempotent and immediately removes bytes
from query eligibility and handle expansion.

Memory is released/overwritten on a best-effort implementation basis; no physical secure-erasure claim
is made.

## `snapshot_overlay_view`

```text
snapshot_overlay_view(scope, live_state, now, limits)
    -> Result<OverlaySnapshot, OverlayError>
```

Returns an immutable bounded view containing exact saved/unsaved revision identities, precedence,
shadow set, access/security generations, TTLs and profile digests. Unsaved bytes remain behind
package-local opaque references.

Expired or unauthorized entries are invalidated before the snapshot is returned.

## `compute_shadow_set`

```text
compute_shadow_set(base_memberships, saved, unsaved, live_state)
    -> Result<OverlayShadowSet, OverlayError>
```

For every source membership, the highest authorized current overlay shadows all older base/saved
revisions. Shadow identity is exact and revision/generation bound; path-only matching is forbidden.

Unknown precedence, authorization or source identity yields fail-closed shadowing plus a typed coverage
gap, never stale base exposure.

## `retrieve_overlay`

```text
retrieve_overlay(request, snapshot, direct_ports, budget, cancel)
    -> Result<OverlayCandidateSet, OverlayError>
```

Executes only bounded in-memory/direct exact, token and accepted structural operations over entries in
the immutable overlay snapshot. It does not create a durable index or call Qdrant.

Every candidate binds source/snapshot revision, native anchor, profile/assurance, access/security fence
and non-content ranking metadata. Unsaved candidate excerpts are returned only within current binding and
disclosure ceilings.

Budget/cancellation/provider degradation produces explicit gaps. It never unshadows older base
candidates as a fallback.

## `merge_overlay_and_base`

```text
merge_overlay_and_base(base_nominations, overlay_candidates, shadow_set, fusion_profile)
    -> Result<CandidateInputSet, OverlayError>
```

Removes all shadowed base nominations before fusion, preserves overlay precedence and uses deterministic
candidate identity/order. It never compares raw scores from incompatible populations or marks an
unvalidated overlay candidate as final evidence.

Candidate validation/live-security recheck remains mandatory downstream.

## `prepare_save_admission`

```text
prepare_save_admission(unsaved_guard, observed_saved_revision, current_auth)
    -> Result<OverlaySaveTransition, OverlayError>
```

Does not persist bytes. It proves the observed saved revision corresponds to the intended buffer version
or returns a conflict. Durable `SourceRevision` and residency receipts are created by source acquisition/
revision-store owners. Only after their accepted receipt may the unsaved overlay retire in favor of a
saved overlay.

## `recover_saved_overlay`

```text
recover_saved_overlay(revision_refs, live_state, preparation_ports, budget)
    -> Result<SavedOverlayRecovery, OverlayError>
```

After daemon restart, only saved overlays may be reconstructed from immutable admitted revisions.
Unsaved bytes and guards are gone; stale non-reconstructive metadata is used only to invalidate old
handles/sessions, never to recreate content.

## Configuration functions

For the `overlay` section:

```text
section_descriptor()
compiled_defaults()
validate_section(section)
section_digest(section)
plan_section_change(old, new)
apply_live_change(validated_change)
```

Quota/TTL reductions invalidate excess/expired entries before the new settings receipt is published.
Increasing limits cannot restore closed/revoked/expired snapshots. No setting may permit durable unsaved
bytes, crash-dump inclusion, plaintext telemetry or a persistent secondary index.

## Cancellation, deadlines and crash semantics

- attach/replace must be atomic with shadow publication; uncertain outcome is resolved from the
  package-local snapshot/guard identity before retry;
- query cancellation releases request-local references but does not invalidate the editor snapshot;
- disconnect/revocation/shutdown invalidates the binding's unsaved state;
- daemon crash destroys unsaved bytes and pins; no recovery claim reconstructs them;
- saved overlay remains reproducible from immutable revision receipts;
- partial overlay retrieval preserves shadows and reports incomplete coverage.

## Typed failures

- `UNSAVED_BUFFER_UNOBSERVED`
- `UNSAVED_BUFFER_UNAUTHENTICATED`
- `UNSAVED_SNAPSHOT_NOT_ADMITTED`
- `UNSAVED_VERSION_CONFLICT`
- `OVERLAY_QUOTA_EXCEEDED`
- `OVERLAY_EXPIRED`
- `OVERLAY_AUTHORIZATION_LOST`
- `OVERLAY_OWNER_GENERATION_CHANGED`
- `OVERLAY_PURGED`
- `OVERLAY_BUDGET_EXHAUSTED`
- `OVERLAY_RETRIEVAL_CANCELLED`
- `OVERLAY_PRECEDENCE_UNKNOWN`
- `OVERLAY_SAVE_CONFLICT`
- `DURABLE_UNSAVED_FORBIDDEN`

## Required tests / qualification evidence

- exhaustive sink audit: unsaved bytes absent from redb, CAS, Qdrant, logs, metrics, backups, restore
  manifests, crash attachments, provider caches, evaluation corpora and learning inputs;
- authenticated attach and binding/sensitivity ceilings;
- precedence `unsaved > saved > published` under concurrent replacement;
- close, replace, TTL, disconnect, revocation, purge and owner-generation invalidation;
- daemon restart destroys unsaved content and invalidates tokens;
- quota/TTL reduction invalidates before settings receipt;
- overlay budget/cancellation never exposes shadowed stale base;
- saved overlay references a real immutable revision and can recover after restart;
- explicit save transition requires matching durable revision receipt;
- durable source handle/continuation to unsaved content is rejected;
- deterministic shadow set and candidate ordering;
- public/debug serialization contains no unsaved bytes or reconstructive metadata.
