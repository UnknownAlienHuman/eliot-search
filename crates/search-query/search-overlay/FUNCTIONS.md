# search-overlay — source audit and optional replacement

## Current source and caller reality

- `src/lib.rs` is 62,649 bytes of production Rust.
- Repository search for `search_overlay::` found the package's own integration test; no daemon/query/LSP
  production consumer was found.
- The README claims `SOURCE_PRESENT`, but source presence and self-tests do not make a shipped overlay.

The current API combines:

```text
saved-overlay admission and recovery
unsaved snapshot attach/replace/invalidate
caller authentication/access/purge/owner observations
operation identity and idempotency receipts
TTL/quota/config digest/change planning
shadow-set compilation
custom exact/token candidate retrieval
base/overlay ranking merge
save transition
handle/continuation constraints
```

This duplicates owners which are being removed elsewhere and would force the direct query back through the
old grant/plan/receipt architecture.

## Baseline action

Do not implement an adapter to the current API. Remove its workspace/Cargo consumers and delete the package
when the caller census remains test-only.

The baseline query uses committed `PublishedView` evidence only. It reports editor overlay as unsupported
rather than silently searching stale committed data under an overlay claim.

## Optional direct model

Only after #279's source-backed query works and a concrete editor adapter is selected, implement beside the
query/session owner:

```text
struct EditorOverlay {
    session_id
    source_id
    editor_version
    position_encoding
    bytes
    expires_at: Instant
}

put_full_snapshot(session, source, version, encoding, bytes, context)
close_snapshot(session, source)
query_overlay(session, selected_sources, predicate, context) -> bounded overlay nominations/evidence
commit_saved_revision(session, source, observed_revision)
```

### Attach/replace

- validate one real session and admitted source identity;
- enforce document and aggregate memory ceilings before copying;
- require monotonically increasing editor version;
- atomically replace one map entry;
- never serialize bytes or reconstructive metadata;
- no mutation receipt: success means the process-local map contains the new value.

### Query

- only the same live session sees its entries;
- exact source ID shadows the committed source for that session;
- query the overlay directly with existing bounded literal/identifier primitives;
- charge the normal absolute deadline/cancellation/read/result/output counters;
- mark returned evidence as ephemeral editor content;
- use a small local rank merge only if a real ranked base+overlay product requires it;
- never compare raw scores from unrelated populations or publish overlay content to Qdrant.

### Save

A save notification does not persist the overlay. The filesystem/source owner observes and retains the saved
bytes through the normal ingestion path. When the matching committed revision is visible, drop the overlay.
A mismatch keeps the overlay/gap explicit; it cannot relabel the saved source.

### Lifetime

Close/disconnect/expiry/version conflict/quota failure/restart drops the entry. No durable recovery, lease,
tombstone, cleanup receipt or unsaved handle database. A query window may cache an excerpt only while the
same session and overlay entry are live; otherwise expansion fails expired.

## LSP adapter boundary

Use a mature LSP message/framing library only inside one optional binary. Start with:

```text
initialize
textDocument/didOpen
textDocument/didChange with full text only
textDocument/didSave
textDocument/didClose
shutdown / exit
```

URI is a locator, not source authority. Negotiate position encoding. Reject stale/out-of-order versions.
No incremental patch engine, completion, diagnostics, rename, formatting or second semantic index.

## Delete from current implementation

- `OverlayStore` public authority and all caller-created security/live-state structures;
- saved-overlay recovery and revision receipts;
- generic operation/idempotency records;
- config section digest/change planner;
- custom full scanner/token/ranking framework;
- durable handle/continuation/purge integration;
- fixture matrices protecting those APIs.

## Verification if optional overlay is implemented

- full snapshot replaces atomically and stale version fails;
- memory/document/session/expiry limits apply before retention;
- bytes absent from redb/object store/Qdrant/log/output not explicitly requested;
- session isolation and disconnect/restart cleanup;
- shadowed base source cannot leak for that session;
- save transitions only after actual committed revision observation;
- same absolute request deadline/cancellation and bounded output;
- no production dependency when the feature is not shipped.
