# search-continuation — implemented functions

## Current API

| Entry point | Actual behavior |
|---|---|
| `ContinuationStore::create(request)`, `create_with_security_scope(request, scope)` | Validate supplied records/token material and insert an in-memory window or replan descriptor. |
| `resolve(credential)` | Looks up the record and checks binding/status. |
| `resume(credential, live, now, max_items)` | Checks supplied live fences and returns `ResumePlan`: selected references plus a pin effect, a durable replan descriptor, or exhaustion. |
| `bind_durable_emission` | Binds a caller-supplied durable selection; does not execute the replan. |
| `revalidate_emission`, `commit_emission` | Check a selection before output, then account for delivered fingerprints/revision after output. |
| `complete`, `invalidate`, `invalidate_restart`, `expire`, `apply_live_limits` | Change lifecycle state and return cleanup effects for the caller to perform. |
| `compact_terminal(max_items)` | Remove terminal metadata/token entries with no pending security cleanup. |
| `begin_security_invalidation`, `begin_binding_invalidation` | Start a cursor with `advance`, `complete_cleanup` and `finish`. |

Sources: [lib.rs](src/lib.rs), [emission.rs](src/emission.rs), [security.rs](src/security.rs).
The store does not persist descriptors, run query I/O or own the referenced epoch guards.
A returned effect or supplied live-state boolean does not establish that its resource operation happened.

## Repeated work to remove

`resume` starts from `candidates.iter()` and skips issued fingerprints on every page.
`prepare_emission` scans the candidate prefix again to determine completion; both pre-output
revalidation and post-output commit call it. With N unique candidates delivered one at a time, repeated
prefix visits grow quadratically. Keep an offset in one ordered window and advance after delivery.
The current shared code clones selected items, not the full window; terminal transitions already free
the candidate vector and issued set.

The public daemon already has the useful offset mechanics in
[ContinuationCatalog](../../../bins/eliot-searchd/src/continuation/kernel/catalog.rs) and
[PreparedPage::deliver](../../../bins/eliot-searchd/src/continuation/kernel/catalog/prepared.rs).
Its next-page path does not rerun search or clone the entire window. It still repeats catalog work:

1. `prepare_continue_page` calls [source_fence::digest](../../../bins/eliot-searchd/src/source_fence.rs),
   which enumerates every source, serializes their summaries into a temporary vector and hashes it.
2. [ResultHandleCatalog::prepare_mint_page](../../../bins/eliot-searchd/src/result_handles/kernel/catalog.rs)
   enumerates all sources again and builds a map.
3. The same handle preparation calls `source_fence::digest` again.

Thus a prepared page incurs three source enumerations before delivery, independent of page size.
Use the pinned published view and direct source/revision lookups; an unrelated source change must not
be tested by rebuilding a corpus-wide fingerprint for every existing page.

Consolidate through the [one temporary owner](README.md#one-temporary-owner), changing
[public query callers](../../../bins/eliot-searchd/src/public_runtime_service/kernel/query.rs) and native
lifecycle consumers together. Keep actual scope/revocation and retained-resource cleanup. Remove unused
durable descriptors, duplicate stores and per-page fingerprint receipts with their last consumers.
