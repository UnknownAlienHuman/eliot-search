# Query and source-backed results

The connected query owner turns one typed operation into bounded, source-backed output. It pins one
`PublishedView`, resolves one non-widening scope, calls the concrete handler, preserves the final order and verifies
selected evidence against immutable retained objects.

## Direct flow

```text
typed operation + explicit/default source selection + limits
→ pin current PublishedView
→ resolve ResolvedScope without widening
→ reserve finite candidate/output/window capacity
→ execute Search | Exact | Continue | Expand
→ select page before retained-source I/O
→ point-read evidence in the pinned redb snapshot
→ verify retained representation/range
→ emit source/revision/location/excerpt, match basis and truthful gaps
```

A citation proves provenance. It does not prove relevance, uniqueness, semantic identity, corpus completeness or
behavioral equivalence.

## Search — exact Qdrant 1.20 boundary

Baseline Search is one Qdrant 1.20 `TextQuery` against the `retrieval_text` scoring full-text index named by the
pinned view. The bridge owns vendor types. Its equivalent Rust request is:

```text
QueryPointsBuilder(collection)
  .query(Query::new_text(TextQueryBuilder(query).k(k).b(b)))
  .using(retrieval_text_field)
  [.filter(candidate_filter)]
  [.params(SearchParamsBuilder.idf(base_population_filter))]
  .limit(prefetch_ceiling)
```

The selected exact patch exposes `TextIndexParamsBuilder::bm25_scoring`, `TokenizerType::Code`, `TextQueryBuilder`,
`QueryPointsBuilder`, `SearchParamsBuilder::idf` and `UpdateMode::InsertOnly`. Compile this exact surface in the
implementation cut; documentation inspection is not a build result.

`TextQuery` is any-term BM25 nomination. Search text is not a Boolean or phrase parser. Required/excluded terms
exist only when the typed request explicitly supplies them; they become filter clauses. Exact spelling and
punctuation-sensitive predicates use Exact, not a stricter interpretation of free text.

Preserve Qdrant's returned order. Do not re-sort a single population by raw score, compare scores from incompatible
populations or call exact count to make top-k look exhaustive. Native prefetch/RRF/grouping is optional only when a
real multi-branch indexed request exists. A non-Qdrant branch may use one small private deterministic rank merge;
no public ranking framework is required.

## Scope and IDF

`ResolvedScope` has two filter levels:

```text
base population = source selection ∩ real access/session restriction
candidate filter = base population + request-only path/kind/topical narrowing
idf.corpus       = base population
```

Default or explicit source selection is part of the BM25 population. Query-only narrowing does not redefine IDF.
Both filter forms use indexed payload fields. Explicit empty returns empty without RPC. Unknown/foreign or
incomplete selection fails or reports a gap; it never widens to all sources.

For Stage 2's one-file collection, base population equals the entire physical collection: omit both filter and
`idf` instead of sending ceremonial predicates. For Stage 3A, one compatible physical collection may contain
several sources; source subset and restricted-session selection appear in both candidate eligibility and IDF.

Different physical collections or incompatible retrieval profiles remain separate populations. Do not sort their
raw scores together. The baseline uses one shard because full-text BM25 statistics are shard-local; no per-source
custom-shard portfolio or local fusion owner is selected.

## Evidence and coordinates

Qdrant payload is nomination/routing data, not source evidence. Resolve each selected `(source_id, point_id)` in the
pinned redb snapshot, validate complete identity/profile metadata, then open the exact retained object and range.
Missing, foreign, corrupt or mismatched evidence produces a gap/error, never a card and never a fallback source.

Text locations bind a retained representation plus half-open byte range `[start, end)`. Display lines are 1-based;
columns use UTF-8 byte offsets unless explicitly labelled otherwise. Mapped/extracted text carries its source map
and loss/gaps. Prepared offsets are never presented as raw-source offsets.

Select the page before object reads. Group ranges from the same object where useful, but charge actual integrity and
source bytes. A whole-object hash/AEAD check can cost more than the excerpt; top-k does not make integrity O(page).

## Exact, orientation and comparison

- **Exact** scans the stated pinned retained inventory independently of Search candidates. Complete no-match requires
  complete scan of the declared representation/domain with no gaps.
- **Orient** appears only with real path/heading/definition/import/test/doc observations.
- **Compare** selects actual source-backed subjects first and reports only supported axes. Equal names or lexical
  scores do not establish equivalent behavior.

A text occurrence is not necessarily a definition; syntax is not compiler-resolved call/reference truth. Missing
producers remain unsupported/unknown rather than being inferred from filenames or top-k.

## Request lifetime and windows

One `RequestContext` carries the pinned view, resolved scope, absolute deadline, cancellation and finite counters
through Qdrant, evidence reads, formatting and output. No nested phase resets the timeout. No redb writer,
view-manager mutex or window mutation borrow spans backend/object/process/model I/O.

One-shot output exposes no reusable locator. A live serviced session may transfer the same view guard and final
ordered references into one bounded `QueryWindow`. Continue pages stored final references; Expand reads bounded
context around an emitted item. Cursor advances only after complete bounded output.

## Current cutover

Replace disconnected recipe/capability/grant/plan/ticket/handle/oracle paths with direct composition. The likely
small survivors are scope resolution, request counters, bridge translation, evidence point reads, retained range
validation and optional rank merge. Package names are not required boundaries.

Remove old local postings/BM25 and the sparse `qdrant/bm25` Document path. The [lexical owner](../search-lexical/FUNCTIONS.md)
now selects one Qdrant 1.20 scoring full-text profile and no local alias expansion.

## Verification

After code and minimal Rust 1.98 check/Clippy, execute the default CLI against real Qdrant 1.20: one-file whole
collection; multi-source selected IDF; explicit empty/unknown scope; code-tokenizer camel/snake/joined matches;
typed required/excluded filters; punctuation-sensitive Exact; order preservation; missing/corrupt evidence;
timeout/cancellation before and after RPC; old pinned view; bounded output; and actual cold/warm scope cost. R051
changes instructions only.
