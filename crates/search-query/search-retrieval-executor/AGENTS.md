# Agent contract — search-retrieval-executor

You own only `crates/search-query/search-retrieval-executor/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S21.2-S21.3, S30.3, H15, P08.

## Mission

Execute direct, exact, indexed and optional-provider legs under bounded queues, cancellation and deterministic fusion.

## Ownership

- interactive/verification/background lanes
- leg scheduling and cancellation propagation
- baseline direct/Qdrant leg dispatch
- typed extension-leg dispatch for overlay, exact and optional providers
- within-leg and cross-leg fusion orchestration
- partial-result accounting

## Forbidden ownership

- final source validation or admission
- durable query leases/history
- raw-score comparison across scoring populations
- unbounded queue, prefetch or retries
- hard dependency on later overlay, exact or optional-provider implementations

## Allowed dependencies

`search-contracts`, `search-domain`, `search-query-planner`, `search-qdrant-bridge`, `search-lexical`, `search-epoch-pins`, `search-access`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `Executor::execute(plan, context) -> Result<RawExecution, ExecuteError>`
- `Executor::cancel(request_id) -> CancelOutcome`
- `schedule_leg(leg, lane, budget) -> LegTicket`
- `fuse_safe_legs(outputs, fusion_profile) -> FusedCandidates`
- `classify_partial_result(state) -> PartialCoverage`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `RESOURCE_EXHAUSTED`, `QDRANT_UNAVAILABLE`, `INCOMPLETE_COVERAGE`, `CANCELLED`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `foreground work outranks background indexing/model work`
- `cancellation releases pins and stops all cancellable legs`
- `raw scores across partitions never cross fusion boundary`
- `saturation returns RESOURCE_EXHAUSTED or truthful partial`
- `stable plan yields stable ordering`
- `extension legs are injected through typed ports and cannot bypass budgets or access checkpoints`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W4 / P08**
- Soft `src/` target: **9,500 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.
