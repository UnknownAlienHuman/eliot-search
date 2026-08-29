# Function contract — `search-retention`

**Status:** W7/P13 logical contract; no CAS lifecycle, purge or restore implementation exists yet.

This package owns one monotonic lifecycle policy across Search-owned immutable objects, purge fences,
tombstones and restore quarantine. It decides what must remain protected or become inaccessible, but
executes concrete control/CAS/index/handle/cache operations only through vendor-neutral ports.

Ordinary retired Qdrant-point reclamation remains owned by `search-index-reclaimer`. Handle records and
expansion authorization remain owned by `search-handles`. Search never obtains authority to delete
client-owned canonical evidence.

## State ownership

The package owns:

- retention/legal-hold policy decisions and durable retention roots;
- sweep generation, frozen root set, mark manifest, deletion plan/checkpoints and final sweep receipt;
- live purge fence, non-content purge tombstone and multi-layer purge state/receipt;
- paired recovery manifest validation and restore quarantine/admission state;
- lifecycle invalidation requests and client revocation-event description;
- truthful distinction between logical denial, projection deletion, cache/CAS deletion, backup status
  and physical secure-erasure limitations.

It does not own source bytes, CAS serialization/addresses, concrete object/index/control stores, ordinary
index reclaim, handle persistence, client evidence governance or backup-provider implementation.

## Retention roots

Closed baseline durable root kinds:

```text
active_projection_manifest
publication_intent
compensation_intent
retained_source_revision_lease
durable_source_handle
client_pin_import_export_contract
paired_recovery_manifest
retention_policy
legal_hold
purge_tombstone
restore_quarantine_manifest
```

Active query/continuation/route/epoch pins are transient protection inputs captured for each sweep. A
missing required root provider makes the sweep generation incomplete and blocks deletion.

## `validate_retention_policy`

```text
validate_retention_policy(policy, current_control, limits)
    -> Result<ValidatedRetentionPolicy, RetentionError>
```

Validates bounded durations/batches, legal-hold precedence, residency domains, protected root kinds,
backup disclosure and deletion semantics. Zero never means unlimited. A policy cannot weaken purge,
exclude an architecture-required root, enable refcount-only correctness or claim guaranteed secure
erase.

## `collect_durable_roots`

```text
collect_durable_roots(control_snapshot, root_ports, policy, budget, cancel)
    -> Result<DurableProtectionSet, RetentionError>
```

Reads one coherent immutable control generation and enumerates every required durable root through
bounded ports. Each root binds object/manifest/revision identity, residency domain, reason, owning
record/generation and expiry/hold semantics where applicable.

Duplicate roots collapse only by exact object identity and compatible residency domain. Unreadable,
unknown or stale root sources remain explicit and block sweep deletion. Cancellation/budget exhaustion
returns no complete protection set.

## `augment_with_active_pins`

```text
augment_with_active_pins(durable, pin_snapshot, route_snapshot)
    -> Result<ProtectionSet, RetentionError>
```

Adds exact source/object/manifest protection implied by active query, continuation, route and epoch pins.
Pin snapshot must match owner epoch, collection routes and capture time. `UNKNOWN_FAIL_CLOSED`, stale or
mismatched pin state blocks sweep.

Process restart never assumes old process-local pins survived; resumed sweeps capture a fresh pin
snapshot before deletion.

## `begin_sweep_generation`

```text
begin_sweep_generation(protection, policy, control_port, operation)
    -> Result<SweepIntent, RetentionError>
```

Durably records root/control/policy/pin/route generations and exact protection-set digest before object
enumeration/deletion. Same operation identity with equal input reconstructs the intent; conflicting
reuse is rejected.

No object may be deleted under an uncommitted or superseded sweep intent.

## `mark_reachable`

```text
mark_reachable(intent, object_graph_port, checkpoint, budget, cancel)
    -> Result<MarkProgress, RetentionError>
```

Traverses only Search-owned bounded manifest/object edges in canonical order. Every marked object binds
residency domain and object kind. Missing/corrupt manifest edges, cycle/depth/budget exhaustion or
provider errors are recorded and prevent a complete mark.

A partial mark is checkpointable but cannot authorize sweep. Mark output contains IDs/digests only, not
source bytes.

## `checkpoint_mark`

```text
checkpoint_mark(intent, progress, control_port, operation)
    -> Result<MarkCheckpoint, RetentionError>
```

Persists intent/root/graph/profile digests, canonical frontier and marked-manifest refs. Unknown commit
outcome is resolved by operation readback. It never stores source bodies or plaintext secrets.

## `finalize_mark_manifest`

```text
finalize_mark_manifest(intent, completed_progress, current_control, current_pins)
    -> Result<MarkManifest, RetentionError>
```

Requires complete traversal and revalidates control/root/policy/legal-hold/purge and fresh pin
generations. Any drift restarts or supersedes the sweep generation; it never patches a mark manifest by
adding a late root during deletion.

## `plan_sweep`

```text
plan_sweep(mark, object_inventory_port, policy, limits, budget, cancel)
    -> Result<SweepPlan, RetentionError>
```

Computes exact unmarked Search-owned object IDs partitioned by residency domain and bounded deletion
batches. Objects protected by tombstone/audit policy remain as explicit non-content records. The plan
binds object inventory generation and mark digest.

Reference counts may be observations but never the deletion authority.

## `execute_sweep_batch`

```text
execute_sweep_batch(plan, batch, object_admin_port, current_fences, deadline, cancel)
    -> Result<SweepBatchReceipt, RetentionError>
```

Before dispatch revalidates sweep intent, control/purge/legal-hold generation and a fresh pin protection
snapshot as required by policy. Deletes exact object IDs only, then verifies absence or the store's
accepted deletion state.

Cancellation before dispatch is clean. Timeout/cancellation/disconnect after dispatch becomes
`SWEEP_DELETE_OUTCOME_UNKNOWN`; exact readback classifies complete, none-applied, partial or conflicting
state. A different exact ID set cannot reuse the operation identity.

## `resume_sweep`

```text
resume_sweep(intent, mark, checkpoint, current_control, current_pins, ports, budget, cancel)
    -> Result<SweepReceipt, RetentionError>
```

Revalidates all generation/digest identities, exact-readbacks uncertain/completed batches and resumes
only remaining exact IDs. A new root, hold, handle, publication intent, purge generation or active pin
cannot be ignored; it invalidates or narrows the sweep.

## `complete_sweep`

```text
complete_sweep(intent, mark, batch_receipts, final_inventory)
    -> Result<SweepReceipt, RetentionError>
```

Succeeds only when every planned object is exactly accounted and no protected object was deleted.
Receipt states Search-owned CAS/cache deletion only. It never claims security purge, backup deletion or
physical secure erase.

## Purge operations

### `validate_purge_request`

```text
validate_purge_request(request, authorization, control_snapshot, policy)
    -> Result<ValidatedPurgeRequest, RetentionError>
```

Requires an authenticated authorized mutation, explicit purge scope, source namespace/owner generation,
current membership/residency/access state, operation identity and disclosure-safe reason. It separates
Search-owned material from client-owned imported/canonical evidence.

### `install_purge_fence`

```text
install_purge_fence(request, control_port, snapshot_port, operation)
    -> Result<PurgeFenceReceipt, RetentionError>
```

One durable monotonic transaction increments purge/security generation, installs the live deny fence and
non-content tombstone, records scope/owner generation and emits invalidation work. Acknowledgement waits
until the immutable live security snapshot exposes the fence.

If durable commit may have occurred, cancellation cannot report rollback. Recovery completes snapshot
publication/invalidation or leaves the scope `FAIL_CLOSED`.

### `build_purge_plan`

```text
build_purge_plan(fence, durable_roots, ports, policy, limits)
    -> Result<PurgePlan, RetentionError>
```

Produces exact bounded targets for:

```text
projection points/manifests
source handles and continuations
Search-owned caches
Search-owned CAS objects no longer protected by legal/audit policy
recovery/backup manifests or external deletion requests
client revocation events
```

Ordinary index-reclaim receipts are observations only and cannot satisfy the purge layer. Shared objects
reachable from unaffected memberships/holds are not deleted; they remain inaccessible to the purged
scope and protected until no remaining root exists.

### `execute_purge_layer`

```text
execute_purge_layer(plan, layer, ports, deadline, cancel)
    -> Result<PurgeLayerReceipt, RetentionError>
```

Every layer uses exact targets, stable operation identity and readback/verification. Partial or unknown
outcome is recorded; the live deny fence remains effective. Failure of physical/cache/backup deletion
never reopens logical access.

Handle/continuation invalidation is performed by their owner through an invalidation port. Projection
deletion uses the security purge index-admin path and its purge receipt, not ordinary reclaim.

### `emit_client_revocation_event`

```text
emit_client_revocation_event(fence, client_import_refs, event_port)
    -> Result<ClientRevocationReceipt, RetentionError>
```

Sends a typed content-free revocation event for evidence already imported by a client. It does not claim
or exercise authority to delete client-owned canonical evidence.

### `finalize_purge_receipt`

```text
finalize_purge_receipt(fence, layer_receipts, tombstone_readback)
    -> Result<PurgeReceipt, RetentionError>
```

Receipt has independent statuses:

```text
logical_non_accessibility
handle_and_continuation_revocation
projection_deletion
cache_deletion
cas_deletion
backup_snapshot_status
client_revocation_event_status
physical_secure_erasure_limitation
```

Logical denial may be complete while other layers remain partial/unavailable. `SECURE_ERASE_NOT_GUARANTEED`
is an honest limitation, never a failed logical purge. The tombstone persists to block resurrection.

## Recovery and restore operations

### `build_paired_recovery_manifest`

```text
build_paired_recovery_manifest(control_checkpoint, qdrant_snapshot, publication, purge_state,
                               schema_profile, object_roots)
    -> Result<PairedRecoveryManifest, RetentionError>
```

Binds installation incarnation, redb checkpoint digest, Qdrant snapshot identity, collection
schema/generation, committed visible epoch, latest publication receipt, purge-tombstone generation,
object/root manifest digests, configuration/profile/API identities and backup-provider receipt.

An independently created redb or Qdrant snapshot is not a valid pair.

### `validate_restore_manifest`

```text
validate_restore_manifest(manifest, target_installation, accepted_profiles, policy)
    -> RestoreDecision
```

Closed decisions:

- `REJECT_CORRUPT_OR_INCOMPATIBLE`
- `RESTORE_PENDING_REVALIDATION`
- `REBUILD_PREFERRED`

Restore never enters serving state directly. Identity, schema/profile, checkpoint/snapshot pairing,
publication receipt and purge generation must be internally coherent. A different installation
incarnation requires explicit import/new-generation semantics.

### `enter_restore_quarantine`

```text
enter_restore_quarantine(manifest, control_port, process_index_ports, operation)
    -> Result<RestoreQuarantineReceipt, RetentionError>
```

Imports/restores into an isolated non-serving data root/collection generation with no client request
admission. The exact manifest and tombstone generation are durably recorded before any revalidation.

### `revalidate_restore_sources`

```text
revalidate_restore_sources(quarantine, live_registry, source_ports, access_port, budget, cancel)
    -> Result<RestoreRevalidationReport, RetentionError>
```

Revalidates external source identities/owner generations, memberships, current access/residency policy,
source revision availability, collection/schema/profile receipts and every purge tombstone before
serving. Missing/drifted sources become explicit rebuild/drop gaps; they are not trusted from backup.

### `plan_restore_admission`

```text
plan_restore_admission(quarantine, report, current_control, policy)
    -> Result<RestoreAdmissionPlan, RetentionError>
```

Produces exact keep/drop/reacquire/rebuild targets and requires new guarded publication/route admission.
Purged material is excluded before reindex/readback. Old visible epoch is never blindly restored as
current.

### `commit_restore_admission`

```text
commit_restore_admission(plan, publication_receipts, control_port, snapshot_port)
    -> Result<RestoreAdmissionReceipt, RetentionError>
```

Only after exact rebuild/readback/publication and current guard validation may one new route/generation
become serving. Control commit and immutable snapshot publication are required. Failure remains
quarantined and cannot serve indexed results.

## Invalidation operations

### `build_lifecycle_invalidation_set`

```text
build_lifecycle_invalidation_set(change, current_state) -> LifecycleInvalidationSet
```

Produces exact owner/view/access/purge/retention/route/profile scopes for access, handles,
continuations, validators, publication, caches and clients. It carries identities/reasons/generations,
not content or generic broad vendor filters.

### `verify_invalidation_completion`

```text
verify_invalidation_completion(set, owner_receipts) -> LifecycleInvalidationReceipt
```

Each owner reports its own state transition. Missing/failed receipt keeps the affected domain fail-closed
and is exposed in purge/restore/security status.

## Configuration functions

For `retention`:

```text
section_descriptor()
compiled_defaults()
validate_section(section)
section_digest(section)
plan_section_change(old, new)
apply_live_change(validated_change)
```

Only bounded scheduling/batch limits may apply live. Retention-duration, residency, legal-hold, purge,
backup/restore and secure-erasure semantics require a security barrier, explicit command, qualification
or rejection. Configuration never removes tombstones or authorizes restore.

## Cancellation, deadlines, idempotency and crash semantics

- pure validation/planning is deterministic;
- durable sweep/purge/restore mutations use stable operation identities and readback on unknown outcome;
- cancellation before external dispatch is clean; after dispatch it preserves fence/quarantine and
  resolves exact state;
- interrupted mark/sweep resumes from immutable intent/manifest/checkpoint identities;
- purge logical fence survives every later cancellation/failure and dominates ordinary retention;
- restore crash remains non-serving/quarantined until exact admission receipt;
- no retry creates a second purge generation, sweep intent or serving route for the same operation.

## Typed failures and reasons

- `RETENTION_POLICY_INVALID`
- `RETENTION_ROOT_INCOMPLETE`
- `RETENTION_ROOT_GENERATION_CHANGED`
- `PIN_PROTECTION_UNKNOWN`
- `SWEEP_GENERATION_MISMATCH`
- `MARK_INCOMPLETE`
- `MARK_MANIFEST_INVALID`
- `SWEEP_PLAN_INVALID`
- `SWEEP_DELETE_OUTCOME_UNKNOWN`
- `SWEEP_DELETE_PARTIAL`
- `SWEEP_PROTECTED_OBJECT_CONFLICT`
- `PURGE_NOT_AUTHORIZED`
- `PURGE_SCOPE_STALE`
- `PURGE_FENCE_PUBLICATION_FAILED`
- `PURGE_PARTIAL`
- `PURGE_LAYER_OUTCOME_UNKNOWN`
- `PURGE_TOMBSTONE_MISMATCH`
- `SECURE_ERASE_NOT_GUARANTEED`
- `RESTORE_MANIFEST_INVALID`
- `RESTORE_PAIR_MISMATCH`
- `RESTORE_PENDING_REVALIDATION`
- `RESTORE_SOURCE_DRIFT`
- `RESTORE_PURGE_TOMBSTONE_CONFLICT`
- `RESTORE_PROFILE_INCOMPATIBLE`
- `RESTORE_ADMISSION_BLOCKED`
- `LIFECYCLE_INVALIDATION_INCOMPLETE`

## Required tests / qualification evidence

- all architecture-required durable roots and active pins prevent sweep;
- one membership removal preserves objects reachable by another membership/publication/handle/hold;
- refcount mismatch cannot authorize deletion;
- mark cancellation/crash/checkpoint/resume and root-generation drift matrix;
- sweep timeout applied/none/partial/conflict exact-readback matrix;
- newly added root/hold/pin invalidates or narrows an in-progress sweep;
- purge fence is durable/live before acknowledgement or destructive work;
- revocation at every query/readback/emission/handle/continuation checkpoint;
- handle/continuation/candidate/cache invalidation receipt completeness;
- projection security-purge path distinct from ordinary reclaim;
- shared CAS object remains protected while purged scope is logically denied;
- purge crash/failure at every layer never reopens access;
- purge receipt layer status and secure-erasure non-overclaim;
- client revocation event does not claim deletion authority;
- purge tombstone blocks reindex and restore resurrection;
- paired manifest digest/identity mismatch quarantine;
- redb-only/Qdrant-only snapshot cannot serve;
- restore enters quarantine, revalidates sources/access/residency/purge, rebuilds and commits a new route;
- restore cancellation/crash remains non-serving;
- configuration cannot remove tombstone/weaken roots/enable restore;
- fake control/object/index/handle/pin/source/publication/event ports prove no concrete adapter dependency;
- default diagnostics contain no source bytes, secret, query text or unrestricted path.
