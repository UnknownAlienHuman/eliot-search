# Working on query and results

Read [query semantics](README.md), the current [delivery stage](../../docs/implementation/README.md#delivery-sequence)
and the affected owner/callers. Connect the real handler to pinned evidence and bounded output; no recipe/plan,
grant, ticket, scheduler or second result store.

Search uses the [lexical profile](../search-lexical/FUNCTIONS.md) through the private bridge. Exact scans retained
sources independently. Continue/Expand use the [window owner](search-continuation/FUNCTIONS.md). Optional operations
arrive only with source-backed producers; they do not block Search.

[Scope](search-access/FUNCTIONS.md) owns base versus candidate filters. A whole-collection base omits only
redundant base/IDF clauses: request-only narrowing remains. Empty never widens; unknown selection fails.
Preserve backend order and select the page before evidence reads. Top-k is not an absence/completeness claim.

Use original execution deadlines/cancellation/counters throughout. Exact and startup do not need BM25 state.
Retain view guards across work, not shared mutable window borrows or redb/view locks. A session serializes data
requests while continuing to service cancellation/expiry; only complete output commits window progress.

Move callers and delete displaced implementations/fixtures together. After Rust changes run minimal pinned
check/Clippy for actual consumers; runtime and relevance claims require separate execution evidence.
