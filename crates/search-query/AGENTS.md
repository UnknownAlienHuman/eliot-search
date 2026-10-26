# Working on query and results

Read the [architecture](../../docs/architecture/README.md), [implementation spine](../../docs/implementation/README.md)
and family [README](README.md). Connect one typed request directly to the selected operation, Qdrant 1.20 or
retained-source owner, pinned evidence and bounded output.

## Current baseline

`Search` uses one Qdrant 1.20 scoring full-text payload index:

```text
TextQuery over retrieval_text
+ optional candidate filter
+ optional SearchParams.idf(base population)
→ preserve backend order
→ point-read pinned redb evidence
→ verify retained range
→ card or gap
```

The physical collection may be vectorless. The real Rust surface includes `TokenizerType::Code`,
`TextIndexParamsBuilder::bm25_scoring`, `TextQueryBuilder`, `QueryPointsBuilder`, `SearchParamsBuilder::idf` and
`UpdateMode::InsertOnly`. Vendor types remain private to the bridge.

Free text is any-term BM25 nomination, not a Boolean/phrase grammar. Required/excluded terms are explicit typed
filter clauses. Punctuation-sensitive spelling/location is Exact.

## Scope

Resolve against one pinned `PublishedView`:

```text
base population = selected sources ∩ actual session/access restriction
candidate filter = base population + request-only narrowing
idf.corpus       = base population
```

Source/access selection affects both candidates and statistics. Request-only narrowing does not redefine IDF.
Whole-collection selection omits redundant filter/IDF. Explicit empty never widens; unknown/incomplete fails or
reports a gap. Do not compare raw scores across different physical collections/profiles.

## One owner and one context

Use direct handlers:

- `Search` — Qdrant 1.20 `TextQuery` and pinned evidence;
- `Exact` — complete retained-source scan of the declared inventory/representation;
- `Continue`/`Expand` — one bounded live `QueryWindow` holding the same view guard;
- optional orientation/comparison — only with actual source-backed producers.

One `RequestContext` carries absolute deadline, cancellation and finite nomination/evidence/source/result/output
budgets. Reserve before retrieval. No phase resets, public scheduler/tickets, grants, plans, capability proofs,
separate pins, terminal history or second handle store.

No redb writer, view-manager mutex or window mutation borrow spans Qdrant/object/process/model I/O. Query pins current
under a short lock, releases it, runs work, and mutates a window/cursor only after complete bounded output.

## Evidence and ranking

Qdrant payload nominates; redb/object state proves. Missing/foreign/corrupt evidence never becomes a card or
fallback source. Select the page before object reads and preserve Qdrant order. Do not exact-count ordinary top-k,
normalize/re-sort one population or make top-k an absence claim.

Use native Qdrant prefetch/RRF/grouping only for a real multi-branch indexed operation. A non-Qdrant branch may use
one private checked rank merge. No public ranking formula/profile system.

## Removal/currentness rule

Before deleting/inlining a package, repeat an exact current search over manifests, Rust/non-Rust callers,
constructors/path inclusion, features, targets, tests and workflows. Move surviving callers in the same cut.

Remove old local postings/BM25, sparse `qdrant/bm25` Document inference, alias-expansion profiles, recipes,
supplied proofs, tickets/schedulers, raw-score fusion, process-test/oracle adapters and fixture-only compatibility.
Keep package boundaries only for a real dependency/effect or substantial algorithm.

After production code run minimal locked Rust 1.98 check/Clippy for query, bridge, daemon, protocol and evidence
consumers, then focused real Qdrant/evidence/cancellation/scope cases. R051 changes documentation only.
