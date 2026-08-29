# Function contract — `search-source-reconcile`

**Status:** W5/P09 logical contract; no watcher, USN or inventory implementation exists yet.

This package turns lossy observation hints into truthful source-state continuity. Watchers and the
Windows USN journal accelerate work but never become source truth. Concrete OS event acquisition stays
behind vendor-neutral ports.

## State ownership

The package owns:

- per-root observation cursor and provider-incarnation state;
- explicit observation gaps and their causal reasons;
- bounded reconciliation plans, slice checkpoints and inventory revisions;
- confirmed change sets and freshness classifications;
- current-workspace preflight decisions and live-head shadow requests.

It does not own source identity derivation, source bytes, admission policy, revision storage,
publication or overlay bytes.

## `ingest_watch_hint`

```text
ingest_watch_hint(state, hint, limits) -> Result<WatchHintReceipt, ReconcileError>
```

Validates root binding, provider incarnation, cursor shape and bounded metadata. A hint may schedule
work or open a gap; it never confirms create, modify, rename or delete by itself.

Duplicate hint identity is idempotent. Conflicting reuse of one hint identity is rejected. Hint payloads
must not contain source bytes, secret values or unrestricted absolute paths.

## `observe_cursor_transition`

```text
observe_cursor_transition(previous, observed) -> CursorTransition
```

Returns one closed variant:

- `CONTIGUOUS`
- `DUPLICATE`
- `GAP_DETECTED`
- `PROVIDER_RESET`
- `ROOT_REBOUND`
- `INVALID`

Only `CONTIGUOUS` may advance the continuous cursor. Overflow, journal wrap/reset, resume ambiguity,
provider restart, root rebinding or unprovable ordering opens an explicit gap before currentness can be
claimed.

## `open_observation_gap`

```text
open_observation_gap(root, cause, first_untrusted_cursor, control_port, operation)
    -> Result<ObservationGapReceipt, ReconcileError>
```

Durably records the gap/security-neutral freshness fence through a vendor-neutral control port, then
publishes it to the immutable live observation snapshot. Acknowledgement waits until queries can observe
the gap.

Same operation identity plus equal input returns the committed receipt. Unknown commit outcome is
resolved by readback; it never creates a second gap generation.

## `plan_reconcile`

```text
plan_reconcile(reason, roots, prior_inventory, limits, current_state)
    -> Result<ReconcilePlan, ReconcileError>
```

Binds exact root registrations, path/identity policy revisions, previous inventory revisions,
observation gap generations, cursor targets and finite entry/byte/time limits. Ordering is deterministic.

A plan never claims its scope complete until every root and continuation slice is accounted for.

## `execute_inventory_slice`

```text
execute_inventory_slice(plan, checkpoint, inventory_port, identity_port, cancel)
    -> Result<InventorySliceResult, ReconcileError>
```

Enumerates only the bounded plan slice through injected ports. It records final-handle identity,
location key, metadata/digest observations sufficient for change comparison and explicit unreadable or
escaped entries.

Cancellation, deadline or provider failure returns a resumable incomplete slice. No partial result may
close an observation gap or advance the authoritative inventory revision.

## `compare_inventory`

```text
compare_inventory(prior, observed, identity_rules, limits)
    -> Result<ConfirmedChangeSet, ReconcileError>
```

Produces deterministic exact changes:

- created identity/path binding;
- content or metadata head changed;
- rename/path-binding movement;
- delete/unreachable;
- hardlink/alias relation changed;
- reparse/escape denied;
- unreadable/unstable/unknown.

Comparison uses physical/logical identity and verified observations, not pathname heuristics alone.
Every unknown remains explicit and prevents complete currentness for the affected scope.

## `verify_change_set`

```text
verify_change_set(change_set, safe_reader_port, source_identity_port, budget, cancel)
    -> Result<VerifiedChangeSet, ReconcileError>
```

Reopens only changes that require stable byte confirmation. Stable-read and source-identity receipts
remain owned by their producers. The reconciler records references and cannot weaken no-execute,
containment or admission rules.

A failed or cancelled verification leaves the relevant change unresolved and shadowed.

## `commit_reconcile`

```text
commit_reconcile(plan, verified, control_port, snapshot_port, operation)
    -> Result<ReconcileCommitReceipt, ReconcileError>
```

One guarded control transaction verifies root/policy/owner generations, installs confirmed source-head
and path-binding changes, advances inventory/cursor revisions, emits required shadow/invalidation work
and closes only gaps fully covered by the verified plan.

The immutable observation snapshot is published after durable commit. If snapshot publication fails,
the affected roots remain fail-closed for current-workspace claims until recovery rebuilds the snapshot.

## `recover_reconcile_commit`

```text
recover_reconcile_commit(operation, control_port, snapshot_port)
    -> Result<ReconcileCommitReceipt, ReconcileError>
```

Resolves timeout/crash uncertainty by reading operation identity and committed inventory/gap revisions.
It never reruns filesystem enumeration under the same plan and silently labels newer observations as the
old result.

## `classify_freshness`

```text
classify_freshness(scope, observation_snapshot, now) -> ObservationFreshness
```

Returns one of:

- `CURRENT_CONFIRMED` — relevant cursor continuity and completed inventory revision are proven;
- `OBSERVED_WITH_AGE` — explicitly relaxed request with bounded observed age;
- `GAP_DETECTED` — relevant unresolved gap exists;
- `UNKNOWN` — scope/provider/root state cannot be proved.

`CURRENT_CONFIRMED` is never inferred from a quiet watcher or recent timestamp.

## `preflight_current_workspace`

```text
preflight_current_workspace(scope, requirements, state, budget)
    -> CurrentWorkspacePreflight
```

Strict current requests either proceed under an exact observation fence, request bounded reconciliation,
or return `OBSERVATION_GAP` / `RECONCILIATION_REQUIRED`. A relaxed query may proceed only when its
result contract exposes freshness state and age. Exact negative proof cannot use the relaxed path.

Historical/frozen source views may remain available from retained revisions while the live workspace is
gapped; the decision must label that distinction.

## `handle_live_head_mismatch`

```text
handle_live_head_mismatch(candidate_head, observed_head, scope, state)
    -> HeadDriftDecision
```

A mismatch emits a shadow/reconciliation request and returns `DROP`, `DIRECT_REREAD_WITHIN_BUDGET` or
`GAP`. It never emits the stale indexed candidate as current evidence and never mutates publication
state directly.

## Configuration functions

For the `reconcile` section:

```text
section_descriptor()
compiled_defaults()
validate_section(section)
section_digest(section)
plan_section_change(old, new)
apply_live_change(validated_change)
```

Intervals, slice limits and budgets are finite scheduling controls only. Changing them cannot close a
gap, weaken continuity requirements or turn partial inventory into complete. Lower limits affect future
slices and may pause/reschedule safely.

## Cancellation, deadlines and recovery

- before durable gap/commit dispatch: cancellation is clean;
- after possible control commit: outcome is resolved by operation readback;
- inventory cancellation returns a checkpoint and explicit incomplete scope;
- no deadline/cancellation path publishes `CURRENT_CONFIRMED` without completed verification;
- daemon restart reconstructs cursor/gap/inventory state from the control snapshot and schedules startup
  reconciliation; watcher quietness after restart is not continuity proof.

## Typed failures

- `OBSERVATION_GAP`
- `OBSERVATION_CURSOR_INVALID`
- `OBSERVATION_PROVIDER_RESET`
- `RECONCILIATION_REQUIRED`
- `RECONCILIATION_BUDGET_EXHAUSTED`
- `RECONCILIATION_CANCELLED`
- `RECONCILIATION_SCOPE_CHANGED`
- `INVENTORY_INCOMPLETE`
- `INVENTORY_ENTRY_UNREADABLE`
- `SOURCE_HEAD_DRIFT`
- `RECONCILE_CONTROL_OUTCOME_UNKNOWN`
- `OBSERVATION_SNAPSHOT_PUBLICATION_FAILED`

## Required tests / qualification evidence

- startup reconciliation finds missed create/modify/delete/rename;
- watcher overflow, USN reset, resume and provider restart open gaps before acknowledgement;
- duplicate and out-of-order cursors are deterministic;
- bounded multi-slice inventory cannot report complete early;
- cancellation/deadline checkpoint never closes a gap;
- stable-read failure remains unresolved and shadowed;
- live candidate head mismatch never emits stale current evidence;
- historical retained view remains truthfully available during a live gap;
- exact same control operation is idempotent; timeout-after-commit recovers by readback;
- immutable snapshot publication failure enters fail-closed currentness;
- configuration changes never weaken continuity;
- default telemetry contains no source bytes, secrets or unrestricted path.
