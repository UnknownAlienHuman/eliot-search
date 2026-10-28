# Query and source-backed results

A new Search or Exact operation pins one published view and resolves a non-widening scope. Only Search captures
query scoring. Continue/Expand inherit the existing window's view and result selection; they never switch to
current state. Each handler returns bounded source-backed output or truthful gaps.

## Direct flow

```text
new Search/Exact + source selection + limits
→ pin current PublishedView and resolve scope
→ reserve the operation's finite capacity
→ Search: capture scoring and query Qdrant; Exact: scan retained inventory
→ verify the selected retained evidence under the same execution context
→ emit source/revision/location/excerpt, basis and gaps

Continue/Expand + same-session locator
→ admit the existing window under its original expiry
→ use its pinned view and final ordered evidence references
→ prepare a bounded page or emitted-item neighborhood
→ complete terminal output before committing window progress
```

Search selects its output page before retained-object reads. Exact owns its independent scan and coverage;
it does not require Qdrant nominations or a Search scoring profile. The
[window recipe](search-continuation/FUNCTIONS.md) owns later-page acquisition and output accounting.

A citation proves provenance, not relevance, uniqueness, semantic identity or corpus completeness.

## Search — physical versus query profile

The pinned view binds a physical Qdrant 1.20 profile:

```text
vectorless one-shard collection
retrieval_text Text index
Code tokenizer
explicit normalization/stemmer/phrase/HNSW/memory settings
filter-field schema
```

The admitted request separately captures:

```text
QueryScoringProfileV1 {
  k = 1.2
  b = 0.75
}
```

Validate finite `k >= 0` and finite `0 <= b <= 1`. Changing only k/b changes future request scoring and does not
rebuild a collection. A live window stores final ordered references; Continue does not requery under newer k/b.
There is no baseline `score_threshold`.

Equivalent bridge request:

```text
QueryPointsBuilder(collection)
  .query(Query::new_text(TextQueryBuilder(query).k(k).b(b)))
  .using(retrieval_text_field)
  [.filter(candidate_filter)]
  [.params(SearchParamsBuilder.idf(base_population_filter))]
  .limit(prefetch_ceiling)
  .with_vectors(false)
  .with_payload([source_id and only required routing keys])
```

Do not return `retrieval_text` or excerpts from Qdrant; evidence/object reads are authoritative.

## Text and filter semantics

`TextQuery` is any-token BM25 nomination. The Code tokenizer gives ranking queries subwords plus detached joined
camel/snake identifiers, so an exact joined identifier can rank highly without local alias expansion.

Free text is not a hidden Boolean, phrase or negation grammar. If a typed request exposes required/excluded text
clauses, Qdrant full-text filters require **all query subwords in any order**. They do not receive joined identifier
tokens and, with phrase matching disabled, are not exact identifier/phrase predicates. Exact spelling,
punctuation and byte/location requests use retained-source Exact or a separately selected exact keyword producer.

## Scope and IDF

```text
base population = selected sources ∩ actual access/session restriction
candidate filter = base population + request-only path/kind/topical narrowing
idf.corpus       = base population
```

Default/explicit source selection and real access restrictions affect both candidates and BM25 statistics.
Request-only narrowing does not redefine IDF. Every actual filter field is indexed. Explicit empty returns without
RPC; unknown/foreign/incomplete selection fails or reports a gap and never widens.

Stage 2's one-file collection equals the complete base population: omit filter and `idf`. Stage 3A adds only the
filter indexes required by real multi-source/access requests. Different physical collections/profiles remain
separate score populations; do not sort raw scores together. Baseline uses one ordinary shard because BM25
statistics are shard-local.

## Evidence and coordinates

Qdrant payload nominates and routes; it is not source evidence. Resolve `(source_id, point_id)` in the pinned redb
snapshot, validate complete identity/profile metadata, then open the exact retained object and range. Missing,
foreign, corrupt or mismatched evidence produces a gap/error, never a card or fallback source.

Text locations bind retained representation plus half-open byte range `[start,end)`. Display lines are 1-based;
columns use UTF-8 byte offsets unless labelled otherwise. Mapped text carries source maps/loss/gaps. Prepared
offsets are never presented as raw-source offsets.

Select the page before object reads. Group ranges from the same object where useful, but charge actual integrity and
source bytes. Whole-object verification can cost more than excerpts; top-k does not make integrity O(page).

## Ranking and completeness

Preserve one-population Qdrant order. Do not normalize/re-sort raw scores, exact-count ordinary top-k or report
complete candidate scope. Native Qdrant prefetch/RRF/grouping is optional only for a real indexed multi-branch
operation. A real non-Qdrant branch may use one private checked rank merge; no public ranking framework.

## Exact and later product operations

- **Exact** scans the declared pinned retained inventory independently of Search. Complete no-match requires a
  complete gap-free scan of the stated representation/domain.
- **Orient/Compare** appear only with real source-backed producers and report unsupported axes honestly.
- Syntax/name/text observations never fabricate compiler-resolved call/reference truth.

## Lifetime and windows

Each admitted operation carries its pinned view/scope and original execution deadline, cancellation and finite
counters through evidence, formatting and output. Search additionally captures query scoring; Exact does not need
BM25 settings, and Continue uses stored order rather than new scoring. Lower I/O receives execution-only state.
No nested phase resets. No redb writer, view-manager mutex or mutable window/map borrow spans external I/O.
The [window owner](search-continuation/FUNCTIONS.md#operation-ownership) serializes data requests through owned
pending state while cancellation and expiry remain serviced.

One-shot output exposes no reusable locator. A live session may transfer the same view guard and final ordered
references into one bounded `QueryWindow`. Continue pages stored results; Expand reads bounded context around an
emitted item. Cursor advances only after complete bounded output.

## Cutover and verification

Remove disconnected recipe/capability/grant/plan/ticket/scheduler/handle/oracle paths; local postings/sparse BM25;
score normalization; and alias-expansion profile code. Keep only scope resolution, request counters, bridge
translation, evidence reads, range verification and an optional private rank helper if real consumers remain.

After code and minimal Rust 1.98 check/Clippy, execute exact physical schema, explicit resource settings, minimal
payload/no vectors, k/b validation, any-token ranking, all-subword filters, whole/scoped IDF, empty/unknown scope,
Code-tokenizer joined behavior, punctuation Exact, order, evidence failure, timeout/cancellation, old view and
cold/warm cost. R052 changes docs only.
