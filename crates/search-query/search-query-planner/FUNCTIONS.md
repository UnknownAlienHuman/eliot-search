# Query selection — direct handlers, not a plan compiler

The [query owner](../README.md) defines the connected operation. This file maps the old planner to the small amount
of direct selection logic which may survive. Before deleting/inlining the crate, repeat exact current caller search
across manifests, Rust/non-Rust code, path inclusion, features, targets, tests and workflows.

## Direct baseline

The query owner receives:

```text
typed request
PublishedViewGuard
ResolvedScope
RequestContext
constructed handlers
```

Dispatch is direct:

```text
Search   → Qdrant 1.20 TextQuery handler
Exact    → retained-source scan
Continue → QueryWindow page
Expand   → selected-item bounded context
```

Optional Orient/Compare appear only with real producers. Unsupported operations fail before backend/source work.
No recipe normalization, capability set, task/leg plan, grant, dependency proof or plan fingerprint.

## Search branch selection

Baseline Search has one physical collection/retrieval profile and one compatible base population. Use the pinned
view's collection, `retrieval_text` field and explicit profile `k`/`b`. `ResolvedScope` supplies:

```text
candidate filter = base selected source/access population + request-only narrowing
idf.corpus       = base selected source/access population
```

When base equals whole collection, omit both filter and IDF. Free text is any-term `TextQuery`; typed required or
excluded clauses become explicit filters. Exact punctuation/location never silently falls back to Search.

For actual multiple physical collections or incompatible profiles, produce a finite ordered branch list. Each
branch uses the same absolute deadline and finite nomination ceiling. Raw scores are different populations and are
not directly compared. Add one explicit bounded rank merge only when a real product operation needs it.

## Remove

Delete V1 recipe tables, public capability/task/leg/compiled-plan types, grants, overlap/drift/dependency proofs,
relative per-leg timeouts and fixtures which only seal those records. Move any surviving tiny grouping/check-limit
helper into query composition; delete the crate if no substantial independent algorithm remains.

## Verification

After code and minimal check/Clippy, verify Search/Exact dispatch, unsupported mode before I/O, whole-collection
scope omission, selected-source base/candidate/IDF split, explicit empty/unknown no widening, one absolute context,
TextQuery order preservation and deterministic multi-population behavior only where a real branch exists. R051
changes documentation only.
