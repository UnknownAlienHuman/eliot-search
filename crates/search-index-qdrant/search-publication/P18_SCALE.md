# P18 advanced-scale supplement — `search-publication`

**Status:** blocked. Existing publication `FUNCTIONS.md` and visible-epoch invariants remain normative.

## Migration state

```text
SCALE_PLANNED
CANDIDATE_CREATED
BASE_BUILT_AT_R0
CHANGELOG_CATCHING_UP
FINAL_BARRIER_ENTERED
VALIDATED_AT_R1
ROUTE_SWITCH_COMMITTED
OLD_ROUTE_DRAINING
OLD_ROUTE_RECLAIMABLE
COMPLETE
```

Recovery states are `CANDIDATE_FAILED`, `CANDIDATE_DISCARDED`, `ROLLBACK_PENDING`, `ROLLED_BACK`,
`SCALE_BLOCKED` and `QUARANTINED`.

## Required operations

```text
persist_scale_intent(plan, control, mutation) -> ScaleIntent
record_base_at_r0(intent, manifest, control) -> ScaleBaseReceipt
apply_ordered_catch_up(intent, change_log, candidate, context) -> CatchUpReceipt
enter_final_barrier(intent, guards, control) -> FinalBarrierGuard
validate_candidate_at_r1(intent, candidate, baseline, context) -> ScaleValidationReceipt
commit_route_switch(validated, control, mutation) -> ScaleRouteCommit
recover_scale_intent(intent, control, routes, context) -> ScaleRecoveryReceipt
rollback_scale_intent(intent, baseline_route, control, context) -> ScaleRollbackReceipt
emit_old_route_manifest(commit) -> RetiredRouteManifest
```

## Invariants

- one unresolved global correctness-path publication/migration intent;
- ordered change-log catch-up loses/duplicates no accepted source change;
- final barrier freezes the exact R1 fence before validation/commit;
- guarded redb route transaction is the only serving linearization point;
- Qdrant alias is never commit;
- failed candidate cannot hide/retire baseline points;
- post-switch rollback is a forward guarded route transition, not epoch rewind;
- every transition is crash/reopen tested.

## Required evidence

Kill/reopen at every state and acknowledgement boundary; R0/R1 manifests; guard race; catch-up ordering;
failed-candidate exact discard; route-switch snapshot publication; post-switch rollback; no staged
visibility; old route emitted only after committed switch.
