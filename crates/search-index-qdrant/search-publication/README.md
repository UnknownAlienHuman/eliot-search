# search-publication

C16: the existing pure, vendor-neutral publication coordinator and recovery decisions.
The module does not execute redb/Qdrant calls, persist epochs, verify live fences or provide a
working daemon pipeline. Composition must supply authoritative port results. No product or gate
acceptance is implied by these state transitions.

## Consumed epochs

`PublicationCoordinator::new` requires both `visible_epoch` and `last_reserved_epoch`.
The latter includes aborted and abandoned reservations and must be reconstructed from verified
control history. It is never inferred from VisibleEpoch. `new` is only for a resolved checkpoint;
`restore_inflight` below keeps unresolved work in the sole occupied coordinator slot.

`submit` advances the reservation floor only after input validation and single-flight checks.
`finalize_aborted` releases the active slot but neither lowers that floor nor advances visibility.
After aborting epoch 1 at visible epoch 0, the next reservation is epoch 2, not 1. Exhaustion rejects
before changing either the slot or the floor. The coordinator is not Clone; immutable transaction
snapshots remain cloneable for inspection, not as another owner.

This local accounting does not persist the floor across process death. The journal/owner adapter
must retain it durably before index effects and reconstruct it during recovery. The current redb
schema-3 normal successor/commit APIs still reject skipped epochs. A complete abnormal-finalization
and fencing protocol is required before live composition can commit past an abandoned epoch.
No second ledger is introduced here.

## In-flight restoration

`restore_inflight(PublicationRestoreInput, max_points)` reconstructs an active coordinator and returns
its recovery decision. The composition owner must supply the coherently verified journal route,
original intent/preparation, exact recorded receipt prefix and fresh bound index observations under
the live recovery barrier. It must not synthesize missing operation IDs, receipt references or digests.
The input is transient and redacted in Debug, not another persisted schema or ownership credential.

Restoration does not call `submit`, advance the reserved floor, replay a durable write or touch Qdrant.
The retained target must equal the highest consumed reservation, even when it exceeds VisibleEpoch + 1.
Existing stage/closure/readback/commit validators check the recorded prefix. Fresh observations then
select the existing recovery decision. Contradictory inputs return no usable coordinator.

Changed pre-commit owner/source/membership/access/shadow/purge/profile guards or partial effects lock
the restored slot into COMPENSATING; they cannot be overwritten with new guards to continue forward.
Bare ABORTED and PUBLICATION_BLOCKED remain blocked with the slot occupied. A new owner cannot roll
back a proven committed epoch. Historical guard equality is not live authorization to emit results.

A previous-process SNAPSHOT_PUBLISHED phase is normalized to CONTROL_COMMITTED. The input carries no
old snapshot receipt, and a true `snapshot_published` observation is rejected. Completion and retired
manifest emission require a new current-process snapshot acknowledgement through the existing path.
The current-manifest pointer follows normal coordinator semantics: it installs the new manifest only
at completion, while the occupied committed transaction retains both manifests for recovery.

Nineteen tests in `tests/restore_inflight.rs` cover accepted prefixes, direct continuation, fresh snapshot
acknowledgement, all seven guard changes, owner/route/epoch conflicts, receipt-prefix combinations,
two-sided compensation, blocked terminal labels, point/receipt limits and redacted diagnostics.
They are pure public-API fixtures, not executed process-restart or storage qualification; Rust tests
remain NOT_RUN in this environment. Actual checkpoint/receipt producers and daemon startup wiring,
durable abnormal finalization and skipped-epoch control commits remain unfinished. No shared port,
existing signature, dependency, storage codec or redb-package size is changed by this additive API.

## Immutable point IDs before staging

A physical point ID shared by the current and proposed manifests is allowed only when the complete
manifest entry is identical: identity key, source/projection memberships and all unit, reference,
payload and named-vector digests. Unchanged entries are retained, not staged or closed.

If an entry changes under the same ID, the raw planner diff places it in both create and retire.
Qdrant upsert replaces the existing point, so staging would overwrite old-epoch data before control
commit; the later closure would then close the new point at its own starting epoch. Exact-ID
acknowledgements alone do not make this sequence safe. `submit` rejects it with
`PUBLICATION_PREPARED_INVALID` before reserving an epoch or taking the active slot.

The guard merge-walks the already bounded, sorted entries without allocating another manifest,
lookup map or point set. It compares the complete entries, not only compact IDs or identity-key
fields. Rejection leaves the current manifest, visibility and reservation floor unchanged.
The point-identity/projection owner must provide a valid distinct identity for changed content;
the coordinator does not invent IDs, reinterpret hash algorithms or edit supplied manifests.

Recovery also blocks an already-recorded conflicting plan, even when all stage/closure IDs and
commit/snapshot fields appear to match. It cannot return Continue, PublishSnapshot or automatic
compensation for that plan. Existing evidence is retained for explicit verified repair; this guard
neither repairs historical overwrites nor proves new IDs absent from every historical collection.
Live artifact/schema/identity qualification and exact backend readback are still required.

Backend reference: https://api.qdrant.tech/api-reference/points/upsert-points

## Two-sided compensation

`begin_compensation_plan` returns exact sorted new IDs to remove/exclude and exact old IDs whose
pre-mutation state must be restored. Repeating the call while COMPENSATING returns the same plan.
Normal replacement uses distinct IDs. A same-ID replacement is rejected before staging, rather
than relying on eventual compensation to repair a pre-commit loss of the old version.

`CompensationReceipt` is bound to the transaction and target epoch. `compensate_exact` is sufficient
only if the diff contains no old retired points. Otherwise `compensate_and_restore` additionally
requires a matching `RestorationReceipt` with the complete exact old ID list and no remaining work.
Missing, duplicated, unexpected, wrong-epoch or foreign restoration leaves the transaction active.
A receipt reference is not itself evidence that the external adapter executed or verified anything.

The existing `begin_compensation` list-only projection is retained for create-only callers; using it
does not bypass the required restoration at completion. Ordinary retired-point reclaim and security
purge remain separate. This coordinator does not delete physical points or remove live policy fences.

## Abandonment and recovery

`AbandonFence` names the complete affected projection memberships in addition to exact point
accounting. A point-only filter does not qualify as membership-wide pre-retrieval/pre-IDF exclusion.
The access/control owner must establish and verify the effective durable fence before supplying it.
This API checks the exact membership set, transaction and epoch; it does not authenticate or execute
the producer's scope receipt. Partition-only proofs and invalidation-only commits require their own
complete port protocol.

`recover` rejects unrelated visible epochs, missing or contradictory durable intents, oversized or
duplicate observations, conflicting physical IDs, and incomplete committed readback. Old-point
closures with no remaining new points still select two-sided compensation for nonconflicting plans.
COMPENSATING never becomes forward staging just because the observed ID sets look complete.

A committed decision requires the matching stage/closure/verified/control receipts and exact ID sets.
A snapshot boolean cannot replace its bound acknowledgement. The bare `abandon_fence_durable` hint
returns PublicationBlocked, not CommitInvalidationOnly: a boolean cannot prove complete scope
exclusion. Continue means resume the normal verified steps, not permission to skip readback or commit.

## Compatibility and verification

This guard changes acceptance of invalid same-ID plans, not public signatures, shared contracts,
point-ID derivation, storage codecs, dependencies, lockfile or workflows. It adds no state owner
and does not grow `search-control-redb`. Previously persisted conflicting plans require explicit
repair; they are not silently grandfathered, rewritten, deleted or declared successfully published.

The 21 existing recovery tests are retained, with the two same-ID acceptance scenarios corrected:
one tests valid distinct-ID two-sided compensation; the other proves rejection of an overlapping
diff. Eleven additional tests cover every immutable field, vector-name/value changes, retained
points, fresh/delete-only plans, valid full publication, unchanged state on rejection, competing
submissions, 2,048 small manifest combinations and legacy recovery across phases. Fixtures are
synthetic and exercise the public coordinator API, not live redb/Qdrant or native crash recovery.
The child test module uses an explicit path and is not another Cargo test target.

Rust compilation, tests, formatting and Clippy remain NOT_RUN: Cargo is unavailable in the authoring
runtime (`cargo +1.98.0 test --locked -p search-publication --all-targets` exited 127).
Source hashes and an independent merge-walk check are not executed Rust or independent acceptance.

```sh
cargo +1.98.0 test --locked -p search-publication --all-targets
cargo +1.98.0 check --workspace --all-targets --all-features --locked
```

Source contract: [FUNCTIONS.md](FUNCTIONS.md), S13.1/S13.4/S13.5. Ownership and limits remain in
[AGENTS.md](AGENTS.md). T27 and T09 remain unaccepted until real journal/index composition and the
required executed evidence are present.
