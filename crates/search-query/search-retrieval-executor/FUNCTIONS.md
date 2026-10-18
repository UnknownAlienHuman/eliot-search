# search-retrieval-executor — current API and removal boundary

## Current API

| Entry point | Actual behavior |
|---|---|
| `admit(plan, owner_id, lane, now_tick, scheduler)` | Checks caller-supplied queue counts and plan limits; reserves no queue slot. |
| `schedule_leg` | Checks dependencies/access/deadline fields and clones a planned leg into a ticket. |
| `acquire_leg_pins` | Creates a separate pin set that cancellation of the execution guard does not own. |
| `dispatch_leg` | Calls a generic backend and validates shape/limits; issued tickets remain usable independently. |
| `normalize_within_population` | Sorts raw scores and assigns ranks. |
| `fuse_safe_legs` | Uses floating weighted reciprocal rank, missing-weight defaults and a map keyed by caller candidate ID. |
| `classify_completion` | Summarizes caller execution maps and complete-scope flags. |
| `cancel` | Sets flags and clears output content but does not necessarily stop issued backend I/O or drop separate pins. |
| `execute_indexed_leg` | Builds `candidate:{leg_id}:{point_id}`, calls filtered query and unconditional exact count, then reports a complete denominator. |

The source is about 60.8 KiB across `lib.rs` and `indexed.rs`. The real public daemon does not call the
existing live Qdrant plane through this path.

## Replacement

```text
execute_search(request, published_view, resolved_scope, bridge, request_context)
```

Baseline:

1. check cancellation/deadline and reserve bounded nomination capacity;
2. call Qdrant BM25 with the selected filter and matching IDF corpus;
3. receive at most the requested/prefetch ceiling;
4. preserve returned order and stable point IDs;
5. hand selected hits to evidence resolution under the same context;
6. return truthful non-exhaustive gaps on timeout/limit/backend failure.

For multiple indexed branches, express prefetch and RRF/grouping in the Qdrant Query API. For an actual
non-indexed branch, merge finite ranked lists with one private checked integer RRF helper. No public formula,
profile digest or plan authority.

Use one absolute deadline; every backend/source/output phase consumes remaining time. Keep a few explicit
aggregate counters in the request owner. Counters are runtime limits, not durable receipts or a universal
ledger. A normal request cancellation drops guards/windows/content; no historical terminal map is needed.

## Remove

- public scheduler/guard/ticket/execution-state types and count-equality completion;
- relative leg budgets/deadlines and separately owned pins;
- public compiled-plan/capability/security inputs;
- floating/default fusion and leg-prefixed candidate IDs;
- exact count from ordinary top-k and `CompleteCandidateScope` claims;
- process-test/oracle backend from production composition.

Keep only useful bounded backend translation, optional rank merge and error/gap mapping. Merge them into
query composition if that is smaller.

## Verification

Use the real bridge to check one BM25 query, compatible scope/IDF, finite prefetch, timeout before/after
RPC, cancellation, missing/foreign/duplicate points, native multi-branch RRF when selected, optional local
merge determinism and final order reaching the evidence cards unchanged.
