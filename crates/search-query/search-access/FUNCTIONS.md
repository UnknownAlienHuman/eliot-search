# Source selection — not an access-control platform

This owner resolves the local user's requested/default source selection against one pinned PublishedView.
[Local scope](../../../docs/architecture/LOCAL_SCOPE.md) has no tenants, restricted-client grants or revocation.

## Direct operation

```text
resolve_scope(requested_selection, pinned_view) -> ResolvedScope
```

Omitted selection uses the configured default. Explicit empty stays empty and performs no backend RPC.
Unknown, foreign or incomplete source membership fails or reports the exact gap; it never widens to all sources.
Resolve membership from the pinned published inventory, not a newer live source catalogue.

## Candidate and statistics scope

```text
base population = selected sources in this view
candidate filter = base population intersect request-only path/kind/text narrowing
IDF population = base population
```

Compatible sources form one union population. Entire-collection base selection omits redundant base/IDF clauses,
not remaining request-only filters. Each real filter field needs its actual indexed producer. Keep incompatible
collections/profiles separate; do not sort raw scores together. Exact/evidence use the same selected inventory.

A source edit or selection change affects new requests. An existing window keeps its pinned view and original
selection until expiry/session close. This is coherent historical evidence, not a security authorization lease.

## Code and cutover

The old validate_grant/eligibility/leg/checkpoint/barrier APIs compare supplied authority metadata. Replace their
real query/daemon/Exact/evidence consumers with the direct selection above; remove grants, per-member plans,
predicate digests, security generations, LiveExpansionBarrier and exclusive fixtures. No replacement policy
service, session authentication or public port catalogue is required. Inline a small resolver in query composition.

## Later checks

Default/explicit/empty/unknown selection; union filters; request-only narrowing not changing IDF; whole-base
selection retaining narrowing; old window versus new source state; no widening and no unrelated catalogue scan.
After code run minimal locked check/Clippy for the resolver and its actual consumers. These are not executed results.
