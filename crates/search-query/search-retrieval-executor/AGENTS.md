# Agent contract — search-retrieval-executor

You own only `crates/search-query/search-retrieval-executor/`. Do not edit another package, the root
workspace, shared contracts or architecture. Missing fields use the contract-change process.

The Architecture 8.4 master is not required for ordinary work. This is the package slice.
Traceability only: S21.2-S21.3, S30.3, H15, P08.

## Mission

Execute direct, exact, indexed and optional-provider legs under bounded queues, cancellation and
deterministic fusion without importing a concrete index adapter.

## Ownership

- interactive/verification/background lanes
- leg scheduling and cancellation propagation
- baseline direct/index leg dispatch through typed ports
- typed extension-leg dispatch for overlay, exact and optional providers
- within-leg and cross-leg fusion orchestration
- partial-result accounting

## Forbidden ownership

- final source validation or admission
- durable query leases/history
- raw-score comparison across scoring populations
- unbounded queue, prefetch or retries
- hard dependency on later overlay, exact or optional-provider implementations
- direct dependency on `search-qdrant-bridge`, redb or process packages

## Allowed dependencies

`search-contracts`, `search-domain`, `search-query-planner`, `search-lexical`,
`search-epoch-pins`, `search-access`. Direct/index/provider execution is injected through
vendor-neutral leg ports.

## Required logical surface

- `Executor::execute(plan, context, ports) -> Result<RawExecution, ExecuteError>`
- `Executor::cancel(request_id) -> CancelOutcome`
- `schedule_leg(leg, lane, budget) -> LegTicket`
- `dispatch_leg(leg, ports, budget) -> Result<LegOutput, ExecuteError>`
- `fuse_safe_legs(outputs, fusion_profile) -> FusedCandidates`
- `classify_partial_result(state) -> PartialCoverage`

## Failure surface

Relevant reasons include `RESOURCE_EXHAUSTED`, `INDEX_UNAVAILABLE`, `INCOMPLETE_COVERAGE`,
`CANCELLED` and `LEG_PROVIDER_UNAVAILABLE`.

## Test seams and exit evidence

- `foreground work outranks background indexing/model work`
- `cancellation releases pins and stops all cancellable legs`
- `raw scores across partitions never cross fusion boundary`
- `saturation returns RESOURCE_EXHAUSTED or truthful partial`
- `stable plan yields stable ordering`
- `extension legs cannot bypass budgets or access checkpoints`
- `package graph contains no Qdrant/redb/process adapter edge`

## Size and split guard

- Delivery wave: **W4 / P08**
- Soft `src/` target: **9,500 lines**
- Hard review threshold: **10,000 hand-written Rust lines**
- Request a split before optional providers force concrete dependencies into this scheduler.

## Definition of done

All execution paths are bounded and port-driven, fusion is deterministic and no concrete storage/index
adapter is reachable from this package.
