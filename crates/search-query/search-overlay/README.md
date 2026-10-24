# search-overlay — optional thin editor state or deletion

Overlay support is optional and cannot block retained-source ingestion, Qdrant publication/query, Exact,
paging or rebuild.

## Inspected revision evidence

The established audit measured `src/lib.rs` at **62,649 bytes** and found external Rust use only in the
package's own integration test. The inspected implementation modelled saved/unsaved revisions, authentication,
security generations, receipts, recovery, config changes, TTL/quota, shadow sets, custom retrieval/ranking and
durable handles.

Those size and caller findings are revision-specific, not a permanent current-branch census. Before deletion,
recheck exact current manifests, Rust imports/constructors/path inclusion, features, targets, tests, workflows and
other non-Rust consumers. Do not connect or repair the old framework merely because it remains in the workspace.

## Baseline disposition

If no concrete editor/LSP consumer is selected at the current head, delete the crate, workspace/dependency
wiring and overlay-only fixtures. If a consumer exists, name it and replace only the bounded behavior it needs.

## Small future overlay

A selected editor/LSP adapter may own one process-local map:

```text
EditorOverlay {
  session_id
  source_id / admitted document identity
  editor version and position encoding
  owned bytes
  absolute expiry
}
```

Rules:

1. start with full-document snapshots; no custom incremental-patch engine;
2. keep bytes only in memory—never redb, immutable objects, Qdrant, logs, caches, backups or crash attachments;
3. atomically retain one latest version per session/document and reject stale updates;
4. shadow the committed source only for the same session and exact source;
5. reuse bounded literal/identifier matching under the normal request context; no second BM25/index/ranking
   framework;
6. mark evidence ephemeral and keep expansion inside the same live session;
7. save through ordinary source ingestion, then remove the overlay after the committed revision is observed;
8. close/disconnect/expiry/limit/restart drops the entry and bytes;
9. owning-account child stdio needs no grant/receipt/checkpoint ceremony.

LSP framing and URI/position handling remain inside one optional adapter. Start with initialize, didOpen, full
didChange, didSave, didClose and shutdown/exit. URI is a locator, not source authority. No completion,
diagnostics, rename, formatting or semantic-index platform is implied.

## Removal boundary

Do not retain the old public `OverlayStore`, saved-overlay recovery, operation receipts, config digest/change
planner, custom scanner/ranking model, durable handles or source-shape fixtures. Move a tiny matcher/shadow helper
only when the concrete query/LSP consumer exists.

See [FUNCTIONS.md](FUNCTIONS.md), [AGENTS.md](AGENTS.md) and optional issue
[#336](https://github.com/UnknownAlienHuman/eliot-search/issues/336).
