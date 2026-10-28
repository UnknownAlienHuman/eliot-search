# Retrieval execution — one bounded query

Replace the disconnected scheduler/ticket/leg framework with a direct handler in the query owner:

```text
execute_search(request, RequestContext, bridge) -> ordered nominations + gaps
```

## Request construction

The view provides physical collection/index profile. The admitted request provides the separate scoring profile;
validate its finite/range limits before dispatch. [Lexical](../../search-lexical/FUNCTIONS.md) owns parameter meaning;
[bridge](../../search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) owns exact SDK construction.

Resolve [scope](../search-access/FUNCTIONS.md) once. Entire-collection base omits only its redundant base clause
and IDF override. Request-only candidate narrowing remains; omit the candidate filter only if no clauses remain.
Explicit empty returns without RPC; unknown/incomplete never widens.

Use one TextQuery with bounded prefetch, no vectors and only required evidence-routing payload. Do not return
retrieval_text as evidence, set a baseline score threshold or exact-count ordinary top-k.

## Execution

1. Check the original absolute deadline/cancellation and reserve nomination/output/window capacity.
2. Dispatch through the shared bridge with local deadline and secondary bounded server timeout.
3. Accept only bounded points/payload, preserve backend order and select the output page before object reads.
4. Resolve each point in the pinned evidence snapshot, verify its retained representation/range and return a card
   or truthful gap under the same execution counters.
5. Transfer references to a live window only through complete output; cancellation/failure advances no cursor.

An integer-second server timeout does not extend the local deadline. Dropping the future does not prove remote
execution stopped; reads create no durable mutation or receipt. Do not retry under a fresh request deadline.

RequestContext owns view, scope, scoring and execution state. Lower I/O borrows execution-only deadline/cancellation/
limits and never renews them. No writer/view/window mutation lock spans backend/object/process I/O. A small RAII
semaphore may bound concurrency; no ticket authority or terminal-history map.

## Semantics and ranking

TextQuery is tokenized nomination, not a Boolean/phrase grammar or exact identifier check. Explicit full-text
filters retain their token semantics; punctuation/exact locations use Exact. One population preserves Qdrant order.
Multiple indexed branches may use native bounded fusion only for a real operation; a non-Qdrant branch may justify
one private rank helper, not a public ranking framework.

## Remove and verify

Delete public scheduler/ticket/execution-state types, separate pins, compiled-plan/capability admission, relative
budgets, unconditional count, default raw-score fusion and oracle/process-fixture adapters with callers.

After production changes run minimal locked Rust 1.98 check/Clippy for actual consumers. Focused cases include
whole base plus request-only filter, scoped IDF, empty/unknown selection, response limits, timeout/cancellation,
foreign points, evidence failure and final order. The [registry](../../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status)
records what has actually run.
