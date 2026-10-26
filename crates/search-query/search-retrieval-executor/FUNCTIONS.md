# Retrieval execution — one `TextQuery`, one request context

The current executor package is a disconnected scheduler/ticket/leg framework. The target product needs one direct
function and may inline it into query composition:

```text
execute_search(request, view, scope, bridge, context) -> ordered nominations + gaps
```

## Exact baseline request

The Qdrant Rust client 1.20 surface inspected at tag `v1.20.0` supports the selected operation:

```text
QueryPointsBuilder::new(view.physical_collection)
  .query(Query::new_text(TextQueryBuilder::new(request.text).k(k).b(b)))
  .using(profile.retrieval_text_field)
  [.filter(scope.candidate_filter)]
  [.params(SearchParamsBuilder::default().idf(IdfParams {
      corpus: Some(scope.base_population_filter)
  }))]
  .limit(request.prefetch_ceiling)
```

Omit filter/IDF when the whole physical collection is the selected population. Request no vectors and only the
bounded routing payload required for point/evidence lookup. The bridge owns these vendor types and validates that
the live schema matches the pinned retrieval profile.

Execution order:

1. check the absolute deadline/cancellation;
2. reserve bounded nomination/output/window capacity;
3. issue one `TextQuery` under the remaining deadline;
4. receive at most the fixed prefetch ceiling;
5. preserve Qdrant order and generation-bound point IDs;
6. select the output page before retained-object I/O;
7. point-read evidence in the pinned redb snapshot and verify retained ranges under the same context;
8. return cards and truthful non-exhaustive gaps.

`TextQuery` matches any query term. Required/excluded terms exist only as explicit typed filter clauses. Do not
parse Boolean/phrase syntax from arbitrary free text in this owner.

## Context and cancellation

One `RequestContext` owns:

```text
PublishedViewGuard
ResolvedScope
absolute monotonic deadline
cancellation token
remaining nominations/evidence items/source bytes/result bytes/output bytes
```

A small process-local semaphore may bound concurrent requests. Acquire it directly and release through RAII. No
relative per-leg deadlines, owner-issued tickets, queue-count proofs or durable terminal map.

Dropping a future is not proof that server work stopped, but query has no durable mutation. On timeout/cancel, drop
buffers/guards/windows, report current failure/gap and do not retry under a fresh deadline.

## Ranking

One baseline population preserves Qdrant order. Do not exact-count ordinary top-k, normalize scores, prefix
candidate IDs with a leg, or declare complete candidate scope.

When a real multi-branch indexed operation exists, prefer bounded Qdrant Query API prefetch/RRF/grouping. A real
non-Qdrant branch may use one private checked integer rank merge. Raw scores from incompatible populations are never
compared directly.

## Remove

Delete public scheduler/guard/ticket/execution-state types, separate pin sets, compiled-plan/capability admission,
relative budgets, floating/default fusion, exact count, process-test/oracle product adapters and fixtures whose only
subject is that framework. Keep only small bridge translation, gap mapping and an optional private rank helper if
real consumers remain.

## Verification

After code and minimal Rust 1.98 check/Clippy, run one vectorless Qdrant 1.20 text query, whole-collection scope,
selected-source `idf.corpus`, request-only narrowing, finite limit, OR/required/excluded semantics, timeout before
and after RPC, cancellation, duplicate/foreign points, evidence failure and final-order preservation. R051 changes
instructions only.
