# search-publication

C16: the existing pure, vendor-neutral publication coordinator and recovery decisions.
The module does not execute redb/Qdrant calls, persist epochs, verify live fences or provide a
working daemon pipeline. Composition must supply authoritative port results. No product or gate
acceptance is implied by these state transitions.

## Consumed epochs

`PublicationCoordinator::new` now requires both `visible_epoch` and `last_reserved_epoch`.
The latter includes aborted and abandoned reservations and must be reconstructed from verified
control history. It is never inferred from VisibleEpoch. Only a resolved checkpoint may initialize
a new coordinator; unresolved-intent hydration and startup qualification remain separate work.

`submit` advances the reservation floor only after input validation and single-flight checks.
`finalize_aborted` releases the active slot but neither lowers that floor nor advances visibility.
After aborting epoch 1 at visible epoch 0, the next reservation is epoch 2, not 1. Exhaustion rejects
before changing either the slot or the floor. The coordinator is not Clone; immutable transaction
snapshots remain cloneable for inspection, not as another owner.

This local accounting does not persist the floor across process death. The journal/owner adapter
must retain it durably before index effects and reconstruct it during recovery. The current redb
schema-3 normal successor/commit APIs still reject skipped epochs; they are not automatically
widened by this change. A complete abnormal-finalization/fencing protocol is required before live
composition can commit past an abandoned epoch. No second ledger is introduced here.

## Two-sided compensation

`begin_compensation_plan` returns exact sorted new IDs to remove/exclude and exact old IDs whose
pre-mutation state must be restored. Repeating the call while COMPENSATING returns the same plan.
For replacement under the same point ID, removal must precede restoration, and readback must verify
the original payload/vector state and visibility bounds from the immutable old manifest.

`CompensationReceipt` is bound to the transaction and target epoch. `compensate_exact` is sufficient
only if the diff contains no old retired points. Otherwise `compensate_and_restore` additionally
requires a matching `RestorationReceipt` with the complete exact old ID list and no remaining work.
Missing, duplicated, unexpected, wrong-epoch or foreign restoration leaves the transaction active.
A receipt reference is not itself evidence that the external adapter executed or verified anything.

The existing `begin_compensation` list-only projection is retained for create-only callers; using it
does not bypass the required restoration at completion. Ordinary retired-point reclaim and security
purge remain separate. This coordinator does not delete physical points or remove live policy fences.

## Abandonment and recovery

`AbandonFence` now names the complete affected projection memberships in addition to its exact point
accounting. A point-only filter does not qualify as membership-wide pre-retrieval/pre-IDF exclusion.
The access/control owner must establish and verify the effective durable fence before supplying it.
This API checks the exact membership set, transaction and epoch; it does not authenticate or execute
the producer's scope receipt. Partition-only proofs and invalidation-only commits require their own
complete port protocol.

`recover` rejects unrelated visible epochs, missing or contradictory durable intents, oversized or
duplicate observations, and incomplete committed readback. Old-point closures with no remaining new
points still select two-sided compensation. COMPENSATING never becomes ordinary forward staging just
because the observed ID sets look complete.

A committed decision requires the matching stage/closure/verified/control receipts and exact ID sets.
A snapshot boolean cannot replace its bound acknowledgement. The bare `abandon_fence_durable` hint
returns PublicationBlocked, not CommitInvalidationOnly: a boolean cannot prove complete scope
exclusion. Continue means resume the normal verified steps, not permission to skip readback or commit.

## Compatibility and verification

Package-owned API corrections: the constructor's explicit reservation floor; non-Clone coordinator;
transaction pre-visible epoch and retained bounds; compensation receipt target epoch; and complete
membership set in AbandonFence. Callers must supply actual values, not compatibility defaults.
Transaction snapshots are obtained through the coordinator; direct unchecked struct construction is
no longer supported. Shared `PublicationGuards`, storage codecs, Cargo dependencies, lockfile and
workflows are unchanged. No code was added to the near-limit `search-control-redb` package.

21 regression tests were added in `tests/recovery_invariants.rs`; existing tests remain. They cover
abort/non-reuse, explicit checkpoint reconstruction, exhaustion, conflicts, both compensation sides,
same-ID replacement, canonical old-ID ordering, membership fences and contradictory recovery. Their
synthetic receipts exercise pure semantics, not live Qdrant, redb, process restart or power loss.

Rust compilation, tests, formatting and Clippy are NOT_RUN: Cargo is unavailable in the authoring
runtime (`cargo +1.98.0 --version` exited 127). No executed or independent qualification is claimed.

```sh
cargo +1.98.0 test --locked -p search-publication --all-targets
cargo +1.98.0 check --workspace --all-targets --all-features --locked
```

Source contract: [FUNCTIONS.md](FUNCTIONS.md), S13.1/S13.4/S13.5. Ownership and limits remain in
[AGENTS.md](AGENTS.md). T27 and T09 remain unaccepted until real journal/index composition and the
required executed evidence are present.
