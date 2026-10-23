# search-retrieval-executor — direct backend work, not an execution OS

Execute the retrieval operations actually selected for one query and preserve their useful order before
source evidence lookup.

## Current source

`src/lib.rs` is about 19.7 KiB and implements caller queue admission, execution guards, cloneable leg
tickets, separate pin sets, relative per-leg deadlines, mutable execution maps and floating weighted-rank
fusion. `src/indexed.rs` is about 41.1 KiB and adds indexed request validation, unconditional exact count,
process-test ports and candidate IDs containing the leg ID.

The daemon contains an unwired process-test indexed port over an in-memory oracle. No serving handler
constructs or calls it; the `wave4-query` module declaration is already present. These types do not establish
a connected backend or a bounded running operation. The [daemon caller map](../../../bins/eliot-searchd/FUNCTIONS.md#query-publication-and-rebuild-boundaries)
records its actual removal boundary.

## Target execution

The query handler owns one ordinary request context:

```text
RequestContext
  PublishedView guard
  ResolvedScope
  absolute deadline
  cancellation token
  finite backend-hit, source-read, retained-result and output limits
```

Call the concrete backend with remaining time and bounded output. Charge counters before retaining work.
A small daemon concurrency semaphore may bound simultaneous requests; it does not require public scheduler
snapshots, leg tickets, plan digests or durable terminal histories.

Baseline lexical search is one Qdrant BM25 query. Preserve the Qdrant order. For multiple compatible
indexed branches, use Qdrant native prefetch/RRF/grouping so candidates are bounded and fused where they
are retrieved. Add a small local rank merge only when a real external branch such as an editor overlay or
exact source result must combine with indexed output.

## Ranking rules

- backend scores are meaningful only within the exact backend/population/profile that produced them;
- cross-branch merge uses ranks, not direct comparison/addition of raw `f32/f64` scores;
- candidate identity is the stable retrieval point, never `leg_id + point_id`;
- branch identity and rank are contributions, not result identity;
- duplicate/conflicting point observations fail the affected backend result;
- preserve the chosen final order through evidence lookup and card formatting;
- bounded prefetch omissions remain non-exhaustive gaps.

A private deterministic integer RRF helper is enough if a local merge is needed. It has fixed application
constants, checked arithmetic and a stable point-ID tie. It needs no public ranking profile, formula DSL,
canonical digest, golden credential or immutable evidence-key framework.

## Removal

After direct callers switch, remove:

- caller `SchedulerSnapshot`/owner IDs and queue-count admission;
- public/cloneable `ExecutionGuard`, `LegTicket`, `ExecutionState` and terminal maps;
- relative per-leg deadlines and separate pin sets;
- `CapabilitySet`/compiled-plan proof admission;
- floating/default `FusionProfile` and raw-score projector reordering;
- candidate IDs containing leg IDs;
- unconditional `count_exact` and fake complete-candidate scope;
- process-test indexed adapter from the product path.

If direct Qdrant plus a small optional merge is all that remains, inline it into query composition and
remove this package rather than retain an execution framework.

[FUNCTIONS.md](FUNCTIONS.md) maps current APIs. [AGENTS.md](AGENTS.md) governs implementation.
