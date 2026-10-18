# search-continuation — current API and removal boundary

## Shared package

| Entry point | Actual behavior |
|---|---|
| `ContinuationStore::create*` | Inserts an in-memory candidate window or durable-replan descriptor from caller-supplied records/token material. |
| `resolve` | Looks up token digest and checks binding/status. |
| `resume` | Starts at the beginning, skips issued fingerprints and returns a selected list plus caller-executed effects. |
| `bind_durable_emission` | Binds a supplied selection; it performs no replan or persistence. |
| `revalidate_emission`, `commit_emission` | Recompute selection state before and after output and record issued fingerprints. |
| terminal/invalidation/expiry methods | Return cleanup effects for callers and retain/compact separate lifecycle metadata. |
| security/binding cursors | Scan records and apply caller-supplied generation changes. |

The package does not execute retrieval, source I/O, durable persistence or a real view guard.

## Public daemon path

[`cmd_search_page` and `cmd_continue`](../../../bins/eliot-searchd/src/public_runtime_service/kernel/query.rs)
use the separate daemon `ContinuationCatalog`, then mint another handle for every emitted match.

The daemon catalog has the useful mechanics:

- one retained ordered vector and direct `next_index`;
- O(page) slicing instead of rescanning the prefix;
- `PreparedPage::deliver` advances only after complete output.

It also repeats corpus-wide work. A continuation page computes a source-catalog fence, the handle catalogue
enumerates every source into a map, and handle preparation computes the same fence again. Thus one page can
perform three whole-catalog enumerations before output.

## Replacement sequence

1. Define one application `QueryWindow` over a `PublishedView` guard and final ordered evidence refs.
2. Move the daemon's prepared-page/cursor behavior into that owner.
3. Emit excerpts and exact source references directly; optional item locators index this window.
4. Replace `ContinuationHandle` / `SearchSourceHandle` contract fields and all real callers together.
5. Remove whole-catalog page fences, issued-fingerprint history, durable replan descriptors, security
   effect records and separate handle minting.
6. Delete the shared package and obsolete daemon catalogues after the serving path uses the new owner.

A partial frame advances zero items. Restart invalidates every window. No durable continuation recovery is
required for ordinary interactive search.
