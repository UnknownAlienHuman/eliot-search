# Function contract — `search-retrieval-executor`

**Status:** W4/P08 bounded execution contract; no scheduler or retrieval implementation exists yet.

The executor consumes an accepted immutable `SearchTaskPlan` and vendor-neutral leg ports. It never
constructs redb/Qdrant/process adapters and never treats nominated candidates as evidence.

## Admission and scheduling

### `admit(plan, binding_quota, scheduler_snapshot) -> Result<ExecutionGuard, ExecuteError>`

Verifies plan/snapshot fingerprints, deadline, lane, resource ceilings and per-binding/global queue
capacity. Admission acquires only process-local request state; ordinary reads create no durable job or
idempotency row.

### `schedule_leg(guard, leg, dependency_state) -> Result<LegTicket, ExecuteError>`

Checks dependencies, live access permit, budget remainder and lane priority before queueing. Queues and
prefetch are finite. Interactive work outranks verification/background without starving bounded
cleanup.

### `acquire_leg_pins(ticket, pin_port) -> Result<LegPinSet, ExecuteError>`

Acquires only the route/epoch pins required by the leg. Every pin is owned by the request guard and is
released on completion, cancellation, disconnect or failure.

### `dispatch_leg(ticket, ports, cancellation) -> Result<LegOutput, ExecuteError>`

Dispatches direct/index/exact/provider operations through typed ports. Cancellation/deadline propagate
to every cancellable dependency. Leg output contains bounded nominations, ranks, coverage and reasons,
not trusted source excerpts.

## Fusion and contamination

### `normalize_within_population(output, profile) -> Result<RankedLegOutput, ExecuteError>`

Raw scores are comparable only inside the exact scoring population/profile that produced them.

### `fuse_safe_legs(outputs, fusion_profile) -> Result<FusedNominations, ExecuteError>`

Cross-population combination uses the pinned rank-based fusion profile and deterministic tie-breaks.
Raw scores never cross population boundaries. Duplicate lineage/unit handling is explicit and bounded.

### `discard_contaminated(decision, execution) -> ContaminationReceipt`

Drops every influenced leg, its ranks, counts, diversity and trace. It may re-dispatch under a new live
permit within remaining budget or return an explicit gap; post-filtered ordering is forbidden.

### `classify_completion(execution, plan, budget) -> ExecutionCoverage`

Reports executed, failed, cancelled, omitted-budget and unavailable legs. Saturation/deadline may yield
a truthful partial candidate scope, never complete scope or an absence claim.

### `cancel(request_id, reason) -> CancelOutcome`

Idempotently stops admission/dispatch where possible, propagates cancellation, releases pins and
request-local memory, and records only bounded non-content outcome metadata.

## Configuration and failure semantics

Implements `config/sections/scheduler.md`. Live decreases constrain new dispatch immediately and may
cancel excess background work; increases apply only within accepted host/server ceilings. Failures
include resource exhaustion, deadline/cancellation, index/provider unavailable, stale plan, access
revoked and incomplete coverage. Read-only dependency timeout is a typed failed leg, not a mutation
unknown-outcome receipt.

## Required fixtures

Lane priority and bounded fairness; queue saturation; cancellation/disconnect releases all pins;
deadline propagation; no durable read rows; raw-score isolation; deterministic weighted-RRF/tie-break;
contaminated whole-leg discard; stable plan/stable ordering; optional leg cannot bypass access/budget;
package graph/public API contains no concrete adapter.
