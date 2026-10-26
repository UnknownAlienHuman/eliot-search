# Analysis registry

Continuation memory and evidence status. It records inspected facts, selected decisions, published changes and
unrun work; it is not architecture, permission or an implementation prerequisite.

<a id="resume"></a>
## Resume

**Checkpoint:** R052, 2026-10-26 — Qdrant 1.20 physical/resource defaults and exact text/filter semantics frozen.  
**Branch:** `codex/refactor-preparation-20261017`, PR #610.  
**Inspected ELIOT head:** `87c84c24aaac7820f2b418d1cbbde2ef91e4e35a` (R051).  
**Original source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`.

Read [docs/README](../README.md), the [implementation sequence](../implementation/README.md#delivery-sequence),
then only the affected owner recipe, current callers and owning issue.

R052 changes documentation only. Product remains unimplemented. Next implementation is Foundation plus Windows
handle-finalization canary, then exact Qdrant 1.20 dependency/schema/query cut and #264.

## R052 inspected evidence

### Qdrant Rust client v1.20.0

Inspected:

- `TextIndexParamsBuilder` including `disabled_stemmer`, `phrase_matching`, `enable_hnsw`, `memory`, BM25 scoring;
- `KeywordIndexParamsBuilder` including HNSW/memory/prefix/tenant settings;
- `CreateFieldIndexCollectionBuilder` and official keyword-index snippet;
- `QueryPointsBuilder` response selectors;
- primitive conversions for `with_payload(Vec<&str>)` and `with_vectors(bool)`;
- `TextQueryBuilder` defaults and request-time k/b;
- generated Memory enum.

### Qdrant Server v1.20.0

Inspected:

- `TextIndexParams` actual defaults and schema serialization;
- BM25 REST validation (`k >= 0`, `0 <= b <= 1`);
- Code tokenizer implementation;
- full-text scorer/filter implementation (`TextQuery` any token; `MatchText` all filter-query tokens);
- text-statistics behavior and payload-index HNSW/resource fields.

### ELIOT owners checked

Lexical profile, bridge, query owner/executor, control/evidence, implementation/test spine, donor/analysis registers,
configuration/publication/product/release programme metadata.

## Findings and decisions

### F1 — hidden payload-index defaults would create unowned resource behavior

Qdrant text and keyword payload indexes default `enable_hnsw=true` and memory to `Pinned`. R051 did not set either.
This could create additional payload-aware graph work when collection HNSW settings allow it and silently pin large
indexes in RAM.

**Decision:** every physical index profile explicitly sets resource behavior. Stage 2 text index uses
`enable_hnsw=false`, `memory=Pinned`. Stage 3A explicitly selects `Pinned|Cached|Cold` per corpus profile and measures
RAM/cache/disk. Real keyword/filter indexes also disable HNSW and set memory. No hidden defaults.

### F2 — physical schema and query scoring were conflated

R051 placed query-time k/b beside physical retrieval profile and implied profile changes rebuild collections.
Qdrant documents k/b as request-time parameters.

**Decision:** split:

```text
PhysicalTextIndexProfile
  field/filter schema, tokenizer, normalization, scoring-index enablement,
  text composition, shard layout, index residency

QueryScoringProfile
  k, b, semantic version
```

Stage 2 fixes k=1.2, b=0.75 and validates finite upstream ranges. Changing only k/b does not rebuild. A request/window
captures the profile. No baseline score threshold.

### F3 — “no stemmer/phrase” needed explicit builder calls and readback

Omission relies on server defaults and can change silently across versions. `TextIndexParamsBuilder` exposes
`disabled_stemmer`, `phrase_matching(false)`, `ascii_folding(false)` and `enable_hnsw(false)`.

**Decision:** Stage-2 exact text schema is:

```text
Code tokenizer
lowercase=true
ascii_folding=false
phrase_matching=false
stemmer=Disabled
stopwords/min/max/nested absent
scoring=BM25
enable_hnsw=false
memory=Pinned
```

Readiness compares every field. Physical schema change creates a fresh generation.

### F4 — Code tokenizer has different ranking and filter semantics

Official implementation gives ranking queries subwords plus joined camel/snake identifiers. Filter queries receive
subwords only. Full-text `MatchText` requires all tokens in any order; `TextQuery` scores any matching term.

**Decision:** no local identifier aliases. Free text remains any-token nomination. Typed required/excluded text
filters, if exposed, are all-subword conditions—not exact identifiers or phrases. Exact punctuation/name/location
uses Exact or a separately selected exact keyword producer.

### F5 — Stage 2 did not need filter indexes

The one-file collection is the entire selected population; it sends no candidate filter or `idf.corpus`.

**Decision:** Stage 2 creates only the text index. Stage 3A creates source/access/path-kind indexes only when real
multi-source/access queries exist. Prefix/tenant optimization appears only for a concrete query pattern.

### F6 — query response was not minimized explicitly

A Qdrant hit only needs point ID plus evidence-routing payload. Returning vectors or retrieval text duplicates data
and increases response/memory cost.

**Decision:** query sets `with_vectors(false)` and a payload include selector for only routing keys, normally
`source_id`. Excerpts always come from pinned retained evidence/object reads.

### F7 — readiness needed both semantic and resource canaries

Schema/point readback does not prove TextQuery scoring, tokenizer behavior or memory/HNSW settings.

**Decision:** readiness includes exact schema comparison, deterministic TextQuery expected order, Code tokenizer
subword/joined cases, any-token ranking, all-token filter behavior, response projection and k/b validation.

## Current implementation sequence

| Stage | Observable result | Primary owners |
|---|---|---|
| 1 Foundation | root/plain settings; handle-safe initialize/open; one guard; status | #333/#266/#360 |
| 2 Walking skeleton | one file → ObjectRef → retrieval text → explicit vectorless Text index → TextQuery canary/query → retained CLI excerpt | #264 and vertical owners |
| 3A correctness/reference | restart; inventory; explicit filter/residency profile; complete generation/view; budget matrix | #270/#271/#331/#261/#300/#303 |
| 3B conditional optimization | source-local diff/identity/mutation only where 3A budgets fail | #259/#329/#261/#263 |
| 4 Exact/session | retained-source Exact and one framed live QueryWindow | #249/#275/#280/#332 |
| 5 Maintenance/depth | stopped-root maintenance and selected product leaves | #302/#244/#281/#227/#335 |

## Anti-bureaucracy / lock rules

- Issues are owners, not dependency order.
- One manager owns each vertical cut; parallel writes are disjoint.
- No hidden server defaults, backend abstraction, alias manager, custom-shard fusion, watcher ledger, init journal,
  query scheduler/tickets, lexical qualification receipts or operation archive.
- No redb writer/view mutex/window mutation borrow spans object/Qdrant/process/model I/O.
- Complete build + physical-schema comparison + deterministic TextQuery canary + durable evidence commit precede
  view installation.
- Last view drop only wakes cleanup.
- Current-caller search is local to code moved/deleted.
- Minimal Rust 1.98 check/Clippy follows production cuts; broad evidence follows connected behavior.

<a id="evidence-status"></a>
## Evidence status

| Area | Status |
|---|---|
| Qdrant 1.20 builder/server default inspection | CHECKED for named tag/files |
| Exact physical text profile V1 | SELECTED, not compiled/executed |
| HNSW disabled / explicit memory | SELECTED, not measured |
| Code ranking vs filter token semantics | CHECKED upstream, not executed in ELIOT |
| Physical/query-profile separation | SELECTED, not implemented |
| k/b validation/no threshold | SELECTED, not executed |
| Minimal payload/no vectors | SELECTED, not executed |
| Stage-2 no filter indexes | SELECTED, not implemented |
| Windows handle finalizer | SELECTED CANARY, unimplemented |
| Qdrant dependency/server upgrade | NOT DONE; lock remains 1.19 |
| Compiler evidence | R031 materializer + unitizer only |
| Foundation/walking skeleton/current binaries | UNRUN |
| Windows/live Qdrant/restart/relevance/scale/end-to-end | UNRUN |

No donor source inspection proves ELIOT correctness.

## Immutable prior records

- R001–R031: audit/removal/dependency work through `da382b66`.
- R032–R045: canonical documentation reconciliation through `af337ec7`.
- R046: implementation-readiness correction `1eeb4191`.
- R047: active-surface hostile verification `858d1e34`.
- R048: donor/analogue synthesis `fed10c1e`.
- R049: Qdrant 1.20/platform-finalization correction `9edd2c1d`.
- R050: active Qdrant-owner synchronization `a4c23110`.
- R051: exact query/schema API and query/preparation/control reconciliation `87c84c24`.
- R052: explicit index resources, physical/query profile separation and exact tokenizer/filter semantics.

## Next action

Implement Foundation and native Windows finalization canary; run minimal compiler/Clippy; then upgrade the exact
Qdrant client/server/schema/query profile and implement #264. Do not begin hidden-default schema, sparse BM25,
alias expansion, filter indexes without consumers, occurrence matching, sequence/closure or optional profiles first.
