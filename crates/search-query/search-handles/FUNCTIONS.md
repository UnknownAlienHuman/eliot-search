# search-handles — current source and removal map

## Shared package

| Current API/area | Actual behavior |
|---|---|
| handle creation/store | Stores token-digest records and source/revision/range metadata in another in-memory authority map. |
| free/public validation and expansion structures | Caller-shaped records/contexts/permits can be copied and supplied back; the package does not own the current view or serving session. |
| invalidation | Maintains another status/index lifecycle rather than dropping one query window. |
| source inventory | `lib.rs` 24,206 bytes plus `invalidation.rs` 10,834 bytes. |

The public daemon does not use this package.

## Daemon handle path

`ResultHandleCatalog::prepare_mint_page`:

```text
list every source into BTreeMap
→ compute source_fence by listing/hashing every source again
→ for each emitted match:
     validate current source/revision/digests
     allocate random token
     stage ResultHandleRecord
→ insert batch only after output
```

The staged-batch/commit-after-output mechanic is useful. The source scans, token records and separate store
are not.

`prepare_expand`:

```text
check caller-created clean LiveExpansionBarrier
→ recompute whole source fence
→ list/scan every source
→ accept arbitrary [byte_start, byte_end] within whole revision
→ read range and compare returned IDs/digests/length
→ retain exclusive handle borrow through output
```

This can expose unrelated revision content and repeats whole-catalog work for one item.

## Replacement operations

No public handle mint exists. The query-window owner may expose:

```text
WindowItemRef { window_id, item_index }
```

### `prepare_expand(item_ref, context_request, request_context)`

1. resolve the live window and exact successfully emitted item index under an exclusive borrow;
2. check original absolute expiry and optional concrete restricted-session binding;
3. resolve the item's `ViewManifestEntry` from the window's pinned view;
4. derive the permitted neighborhood from that exact entry and configured maximum context;
5. reserve source-read/output budget;
6. read and verify the immutable retained object/range;
7. return `PreparedExpansion` which retains the borrow until complete output or drop.

The request may choose a bounded relative neighborhood; it may not supply unconstrained absolute revision
offsets. The source/revision/range come from the manifest, not a copied handle record.

### Output

Check the same absolute expiry/deadline immediately before writing. Complete writer success returns the
frame. Partial output fails the session/window; there is no post-output acknowledgement or handle mutation.

## Delete after caller cutover

- shared `HandleStore`, records, validation contexts, permits and invalidation indexes;
- daemon `ResultHandleCatalog`, entropy/token hashing, source-map/fence validation and handle records;
- `SearchSourceHandle`, durable handle and retention-lease contracts;
- `LiveExpansionBarrier` clean/purge/revocation Booleans;
- arbitrary full-revision range expansion;
- handle-specific crypto/provider, lifecycle and shape fixtures.

Move exact immutable range reading and exclusive prepared-output behavior into the query-window/evidence
owner. Delete this package and daemon module; do not leave a facade.

## Verification

- normal cards require no handle allocation;
- page/expansion performs no whole-source-catalog enumeration;
- only a successfully emitted item index can expand;
- expansion stays within the configured neighborhood of its exact manifest range;
- wrong/missing retained bytes fail real verification;
- expiry/restart/disconnect/revocation remove availability through the actual window/session;
- partial output does not create new state or acknowledge disclosure.
