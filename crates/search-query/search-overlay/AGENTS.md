# Working on search-overlay

This package is not a connected product owner. Read [README.md](README.md), [FUNCTIONS.md](FUNCTIONS.md) and
the [query cutover](../README.md).

Do not repair, split or integrate the current 62.6 KiB `OverlayStore`. Do not add grants, security
snapshots, operation receipts, config digests, saved-overlay recovery, custom ranking or durable handles.
First confirm a real editor/LSP consumer exists. If not, delete the crate and its workspace/dependency/tests.

A future selected overlay is a tiny process-local map owned beside the actual query/session:

- one full-text latest snapshot per session/source;
- finite memory/document/TTL limits;
- direct bounded matching under the normal `RequestContext`;
- exact session/source shadowing;
- no redb/object/Qdrant/log/cache persistence;
- save through ordinary source ingestion;
- drop on close/disconnect/expiry/restart.

LSP types/framing remain inside one optional adapter and start with full-sync open/change/save/close only.
Do not create an incremental patch engine or semantic-index platform.

If implementation is selected, change the direct query/LSP consumer and remove the old overlay API in the
same slice. Run minimal Rust 1.98 compiler/Clippy for actual changed consumers. Self-tests of the disconnected
store are not a product gate.
