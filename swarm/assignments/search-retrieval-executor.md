# `search-retrieval-executor` implementation packet

**Path:** `crates/search-query/search-retrieval-executor`  
**Capability:** C23  
**Delivery:** W4 / P08  
**Gate:** BLOCKED until query planner, access, index and lexical receipts are accepted  
**Trace:** S19.3, S21.2-S21.3, S30.3, H15, P08  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-query-planner`, `search-qdrant-bridge`, `search-lexical`, `search-epoch-pins`, `search-access`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Execute bounded direct/index/provider legs with cancellation, security checkpoints and deterministic cross-leg fusion; return candidate streams, not belief.

## Owns

- bounded scheduler lanes and per-binding quotas
- leg execution/cancellation/resource accounting
- security checkpoints before/after scoring and IDF
- versioned cross-leg rank fusion and candidate stream receipts

## Must not own

- final source validation or result projection
- raw score comparison across incompatible populations
- durable query job rows for short reads
- unbounded queues, prefetch or source reads
- generative synthesis

## Logical primitives

- ExecutionRequest, ExecutionContext, LegExecutor, CandidateStream, LegReceipt, ResourceUsage, CancellationToken, FusionInput, FusedCandidateOrder, ExecutionReceipt

## Logical operations

1. `execute_plan(plan, context, cancel) -> Result<ExecutionReceipt, ExecuteError>`
2. `execute_leg(leg, context, cancel) -> Result<CandidateStream, ExecuteError>`
3. `checkpoint_access(context, checkpoint) -> Result<(), ExecuteError>`
4. `fuse_authorized_legs(streams, profile) -> FusedCandidateOrder`
5. `discard_and_replan_contaminated_leg(leg, new_access) -> ReplanOutcome`

## Required invariants

- all queues/legs/prefetch/results/resources obey QueryExecutionBudget
- cancellation/disconnect releases pins and work
- revoked scoring population discards whole affected leg
- raw BM25 scores never cross population boundary; cross-leg fusion is versioned rank based
- executor emits candidate proposals only

## Typed failure surface

- `RESOURCE_EXHAUSTED`
- `QUERY_CANCELLED`
- `ACCESS_REVOKED`
- `INDEX_GAP`
- `QDRANT_UNAVAILABLE`
- `INCOMPLETE_COVERAGE`

## Exit tests / evidence

- `bounded_queue_and_prefetch`
- `cancel_releases_epoch_pin`
- `access_checkpoint_matrix`
- `cross_partition_raw_scores_not_compared`
- `contaminated_leg_reexecution`
- `partial_result_has_truthful_coverage`
- `deterministic_fusion_tie_break`

## Suggested internal modules

```text
search-retrieval-executor/src/
  scheduler.rs
  lane.rs
  leg.rs
  direct.rs
  indexed.rs
  checkpoint.rs
  cancel.rs
  fusion.rs
  receipt.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 7,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- If direct and indexed executors gain independent runtime dependencies, split behind one executor port before 8,500 lines; do not create forwarding crates.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.
