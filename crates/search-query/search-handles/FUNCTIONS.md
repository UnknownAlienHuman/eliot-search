# search-handles — current API and removal boundary

## Current implementation

| Entry point | Actual behavior |
|---|---|
| `HandleStore::new(policy)` | Creates in-memory token-digest and ID maps. |
| `mint_ephemeral`, `mint_durable_source` | Validate caller-supplied binding/crypto records and insert into the same process-local maps. The durable variant persists nothing and acquires no retention root. |
| `resolve` | Looks up token digest and compares handle identity/class/revision. |
| `revalidate` | Checks supplied lifecycle/security fields and returns a cloned target. |
| `expand` | Calls a readback port, compares returned metadata and revalidates again. |
| `invalidate`, `expire` | Mark records terminal but keep them in both maps. |
| invalidation cursors | Scan retained records and update supplied security/binding generations. |

Sources: [lib.rs](src/lib.rs) and [invalidation.rs](src/invalidation.rs).

The daemon separately implements
[`ResultHandleCatalog`](../../../bins/eliot-searchd/src/result_handles/kernel/catalog.rs). Public paging
first stores matches in `ContinuationCatalog`, then enumerates all sources and mints one new handle record
per page item. Expansion enumerates the source catalogue again before reading the revision.

## Defects to remove, not harden

- Live quotas do not bound terminal history because expired/invalidated entries remain stored.
- `max_expansion_bytes` is validated but not enforced by the shared store's actual read path.
- Two handle stores and one continuation store own overlapping source/revision/range/expiry facts.
- Per-result token minting copies metadata already retained by the query window/view manifest.
- Supplied digest and authority fields do not establish that actual bytes or external effects were checked.
- The baseline local client does not need a durable cryptographic bearer capability for its own result.

## Replacement

Use one `QueryWindow` containing a pinned `PublishedView`, final ordered `EvidenceRef` values, cursor,
absolute expiry and finite byte/item limits. Emit source-backed excerpts immediately. Optional expansion
uses `(window_id, item_index)` and can read only the selected item's evidence/view under the remaining
deadline.

Move useful exact range/readback checks to that owner, update `SearchSourceHandle` consumers in
`search-contracts`, ports, recipes, evidence/projector/provider codecs and daemon, then delete:

```text
crates/search-query/search-handles/
bins/eliot-searchd/src/result_handles/
```

No durable handle profile or compatibility layer replaces them.
