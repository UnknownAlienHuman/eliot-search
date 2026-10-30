# Query and source-backed results

New Search/Exact pins one published view and resolves the local user's selected sources. Continue/Expand retain
their existing window view and final order. There is no authentication/grant/revocation system in
[the local product](../../docs/architecture/LOCAL_SCOPE.md).

<a id="direct-operations-not-a-recipe-operating-system"></a>
## Direct flow

```text
request → pinned view + selected sources + finite execution budget
→ Search: Qdrant nominations; Exact: retained-inventory scan
→ exact retained evidence verification → bounded complete terminal output
→ local window update only after complete transport output
```

One-shot returns no reusable locator. A live session may retain bounded windows for Continue/Expand; a later
process cannot reuse them. A citation establishes provenance, not relevance, uniqueness or exhaustive coverage.

<a id="search--physical-versus-query-profile"></a>
<a id="text-and-filter-semantics"></a>
## Search meaning

[Lexical](../search-lexical/FUNCTIONS.md) owns text and physical/query profiles;
[bridge](../search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) owns exact SDK mapping and readback.
Do not copy schema/default tables here. Changing request scoring alone does not rebuild the physical index.
Free-text ranking is not a hidden Boolean/phrase/exact-identifier grammar; byte spelling uses retained-source Exact.

## Scope and IDF

[Selection](search-access/FUNCTIONS.md) owns base population versus request-only candidate narrowing.
Default is not explicit empty. Unknown/incomplete selection never widens. The complete collection may need no
base/IDF override, but request-only filters remain. Incompatible score populations are not sorted together.

## Evidence and coordinates

[Evidence](search-candidate-validator/FUNCTIONS.md) resolves backend hints in the pinned metadata snapshot, verifies
actual retained bytes and keeps raw/decoded/extracted coordinate domains distinct. A missing or corrupt object is
not a fallback live pathname. Select the output page before object reads; charge actual integrity/read cost.

## Ranking and completeness

Preserve final ranking. Ordinary bounded top-k is not an exhaustive denominator or complete negative. Do not add
exact-count/refill/proof machinery to make it look complete. [Exact](search-exact/FUNCTIONS.md) scans its declared
retained inventory independently and names representation, coverage and gaps.

## Exact and later product operations

Orient/Compare require actual source-backed producers. Syntax/name/literal observations are not compiler-resolved
semantic facts. Optional features extend this path, not a second index, authority or public planning framework.

## Lifetime and windows

[Window](search-continuation/FUNCTIONS.md) owns paging, emitted-item expansion, expiry and prepared output state.
[Protocol](../search-provider-protocol/FUNCTIONS.md) owns terminal/cancellation races; [CLI](../../bins/eliot-search/FUNCTIONS.md)
owns safe presentation. Owned guards may span I/O; mutable shared borrows and writer/view/window locks may not.
Deadlines/counters pass to lower I/O without resets. Search alone needs scoring; Exact and bootstrap do not.

## Cutover and verification

Move direct query/reader/protocol consumers together. Remove old recipes, grants, tickets, permits, local BM25,
public planner/executor/handle frameworks and raw-score normalization when displaced. Keep useful range/identity/
selection checks. [Minimal commands](../../docs/implementation/WORKING_GUIDE.md#minimal-production-checks) apply after
code; [later cases](../../docs/implementation/TEST_CUTOVER.md) follow the connected product. No new run is claimed.
