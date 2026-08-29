# `search-retrieval-executor` implementation packet

**Path:** `crates/search-query/search-retrieval-executor`  
**Capability:** C23  
**Delivery:** W4 / P08  
**Gate:** BLOCKED until planner, access, lexical and pin handoffs are accepted  
**Trace:** S21.2-S21.3, S30.3, H15, P08  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-query-planner`, `search-lexical`, `search-epoch-pins`, `search-access`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust spelling.

## Mission

Execute direct, exact, indexed and optional-provider legs under bounded queues, cancellation and deterministic fusion without importing a concrete index adapter.

## Owns

- interactive/verification/background lanes
- leg scheduling and cancellation propagation
- direct/index/provider leg dispatch through typed ports
- within-leg and cross-leg fusion orchestration
- truthful partial-result accounting

## Must not own

- final source validation or client admission
- durable ordinary-query leases/history
- raw-score comparison across scoring populations
- unbounded queues, prefetch, retries or source reads
- hard dependency on Qdrant/redb/process or later optional provider implementations

## Logical primitives

- `ExecutionContext`, `ExecutionLane`, `LegTicket`, `LegDispatcherSet`, `LegOutput`, `RawExecution`, `FusedCandidates`, `PartialCoverage`, `IndexQueryPort`, `DirectLegPort`, `ProviderLegPort`

## Logical operations

1. `execute(plan, context, ports) -> Result<RawExecution, ExecuteError>`
2. `cancel(request_id) -> CancelOutcome`
3. `schedule_leg(leg, lane, budget) -> Result<LegTicket, ExecuteError>`
4. `dispatch_leg(leg, ports, budget, cancel) -> Result<LegOutput, ExecuteError>`
5. `fuse_safe_legs(outputs, fusion_profile) -> FusedCandidates`
6. `classify_partial_result(state) -> PartialCoverage`

## Required invariants

- every leg is pre-authorized and budget-bound before dispatch
- cancellation/disconnect releases every request-local pin/resource
- raw scores never cross scoring-population boundaries
- cross-leg ordering uses the pinned rank-fusion profile and deterministic tie-break
- saturation returns typed exhaustion or truthful partial coverage
- no concrete vendor adapter appears in the package graph/public API

## Typed failure surface

- `RESOURCE_EXHAUSTED`
- `INDEX_UNAVAILABLE`
- `LEG_PROVIDER_UNAVAILABLE`
- `INCOMPLETE_COVERAGE`
- `CANCELLED`

## Exit tests / evidence

- `foreground_outranks_background`
- `cancellation_releases_pins_and_stops_legs`
- `raw_scores_never_cross_population_boundary`
- `saturation_is_typed_or_truthfully_partial`
- `stable_plan_yields_stable_ordering`
- `extension_legs_cannot_bypass_budget_or_access`
- `package_graph_has_no_qdrant_redb_process_edge`

## Suggested internal modules

```text
search-retrieval-executor/src/
  context.rs
  lanes.rs
  scheduler.rs
  dispatch.rs
  cancellation.rs
  fusion.rs
  partial.rs
  error.rs
```

## Size / split

- Initial `src/` target: **≤ 7,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Request a split before optional providers force concrete dependencies into the scheduler.
