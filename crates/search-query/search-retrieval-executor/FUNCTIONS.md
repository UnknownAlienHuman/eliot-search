# Retrieval execution — one bounded TextQuery request

The current executor package is a disconnected scheduler/ticket/leg framework. The target product needs one direct
operation and may inline it into query composition:

```text
execute_search(request, view, scoring_profile, scope, bridge, context)
  -> ordered nominations + gaps
```

## Exact baseline request

```text
QueryPointsBuilder::new(view.physical_collection)
  .query(Query::new_text(
      TextQueryBuilder::new(request.text)
        .k(scoring.k)
        .b(scoring.b)
  ))
  .using(view.physical_profile.retrieval_text_field)
  [.filter(scope.candidate_filter)]
  [.params(SearchParamsBuilder.idf(scope.base_population_filter))]
  .limit(request.prefetch_ceiling)
  .with_vectors(false)
  .with_payload(vec!["source_id" /* plus only required routing keys */])
```

Omit filter/IDF when the whole physical collection is selected. Validate finite `k >= 0`, finite
`0 <= b <= 1`, finite positive limits and remaining deadline before request construction. Stage 2 fixes
`k=1.2`, `b=0.75`; changing only these request-time values does not rebuild the collection. Do not use a baseline
score threshold.

Execution order:

1. check absolute deadline/cancellation;
2. reserve bounded nomination/output/window capacity;
3. issue one TextQuery under local exact deadline and bounded server timeout;
4. receive at most the fixed prefetch ceiling, no vectors and only routing payload;
5. preserve Qdrant order and generation-bound point IDs;
6. select output page before retained-object I/O;
7. point-read evidence in the pinned redb snapshot;
8. verify retained ranges under the same context;
9. return cards and truthful non-exhaustive gaps.

The local absolute deadline is authoritative. Qdrant's integer-second timeout is secondary and may be rounded only
without extending local admission; dropping the future is not proof the server stopped, but query has no durable
mutation.

## Query/filter semantics

`TextQuery` scores documents containing any query token. Code-tokenizer ranking queries include subwords plus joined
camel/snake identifiers.

Free text is not parsed as Boolean/phrase syntax. If the typed request exposes required/excluded text filters,
Qdrant `MatchText` semantics require all filter-query subwords in any order. Filter queries do not receive joined
identifier tokens. With phrase matching disabled these clauses are not phrases or exact identifiers; Exact remains
separate.

## Context and cancellation

One `RequestContext` owns:

```text
PublishedViewGuard
QueryScoringProfile
ResolvedScope
absolute monotonic deadline
cancellation token
remaining nominations/evidence/source/result/output bytes/items
```

A small process-local semaphore may bound concurrent requests. Acquire directly and release through RAII. No
relative leg deadlines, owner-issued tickets, queue-count proofs or durable terminal map.

On timeout/cancel, drop buffers/guards/window transfer, report current failure/gap and do not retry under a fresh
deadline.

## Ranking

One baseline population preserves Qdrant order. Do not exact-count ordinary top-k, normalize scores, prefix IDs
with a leg, compare scores from different physical profiles or declare complete candidate scope.

When a real indexed multi-branch operation exists, prefer bounded Qdrant prefetch/RRF/grouping. A real non-Qdrant
branch may use one private checked integer-rank merge.

## Remove and verification

Delete public scheduler/guard/ticket/execution-state types, separate pins, compiled-plan/capability admission,
relative budgets, floating/default fusion, exact count, process-test/oracle adapters and framework-only fixtures.
Keep only bridge translation/gap mapping and an optional private rank helper if real consumers remain.

After code and minimal Rust 1.98 check/Clippy, run exact schema/profile, minimal response projection, k/b validation,
any-token query, all-subword filters, whole/scoped IDF, finite limit, timeout/cancellation, duplicate/foreign points,
evidence failure and order preservation. R052 changes docs only.
