# search-overlay — optional thin editor state or deletion

Public main contains a **62,649-byte single-file overlay implementation**, but repository search found its
external Rust consumer only in `search-overlay`'s own integration test. The daemon, query path and LSP do not
construct it. It is not a delivered product capability.

The current package already models a second operating system: saved/unsaved revisions, authentication,
security generations, TTL/config changes, receipts, recovery, quotas, shadow sets, custom retrieval,
ranking merge and durable-handle rules. Do not connect this framework to the new query path.

## Baseline disposition

Search works on committed retained sources first. Overlay support is optional and cannot block source
retention, Qdrant publication, CLI query, paging, exact scan or rebuild.

If no concrete editor integration is selected during the refactor, delete this crate, its workspace
membership/dependencies and overlay-only fixtures.

## Small future overlay

A selected editor/LSP adapter may later own one process-local map:

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

1. full-document snapshots only at first; no custom incremental-patch engine;
2. bytes stay in memory and are never written to redb, immutable objects, Qdrant, logs, caches, backups,
   evaluation or crash attachments;
3. one latest version per session/document replaces the previous value atomically;
4. the overlay shadows the committed source only for that session and exact source;
5. direct bounded exact/identifier matching may reuse existing matcher primitives under the same
   `RequestContext`; no second BM25/index/ranking framework;
6. the normal query result marks overlay evidence explicitly and cannot create a durable `QueryWindow`
   locator after the owning session/overlay expires;
7. save enters the ordinary source-ingestion path; after its committed revision is observed, the overlay is
   removed;
8. disconnect, close, expiry, version conflict, memory limit or daemon restart drops the bytes;
9. an optional restricted-client session may further bind the map; owning-account child stdio needs no
   grant/receipt/checkpoint ceremony.

LSP framing/types remain inside the optional adapter. Use only the messages implemented by the product:
initialize, didOpen, full didChange, didSave, didClose and shutdown/exit. No completion, diagnostics,
formatting, rename or semantic-index platform is implied.

## Removal boundary

Do not retain the current public `OverlayStore`, saved-overlay recovery, operation identities/receipts,
config digest/change planner, caller security snapshots, custom candidate/ranking model or source-shape
fixtures. Move a genuinely useful tiny matcher/shadow helper only when the concrete query/LSP consumer
exists; otherwise delete it.

See [FUNCTIONS.md](FUNCTIONS.md), [AGENTS.md](AGENTS.md) and issue
[#336](https://github.com/UnknownAlienHuman/eliot-search/issues/336).
