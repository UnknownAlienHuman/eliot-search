# Working on search-overlay

Read [README.md](README.md), [FUNCTIONS.md](FUNCTIONS.md), the
[query cutover](../README.md) and optional issue [#336](https://github.com/UnknownAlienHuman/eliot-search/issues/336).

The established audit found a 62.6 KiB overlay operating system with no external production caller at the
inspected revision. Treat that as historical evidence. Before deletion, repeat an exact current search across
manifests, Rust/non-Rust callers, features, targets, tests and workflows.

Do not repair, split or integrate the old `OverlayStore`. Do not add grants, security snapshots, operation
receipts, config digests, saved-overlay recovery, custom ranking or durable handles. First confirm a real
editor/LSP consumer. If none exists at the current head, delete the crate and its wiring.

A future selected overlay is a small process-local map owned beside the actual query/session:

- one latest full-text snapshot per session/source;
- finite document/session/aggregate memory and TTL limits;
- direct bounded matching under the normal request context;
- exact session/source shadowing and explicit ephemeral evidence;
- no redb/object/Qdrant/log/cache persistence;
- save through ordinary source ingestion;
- drop on close/disconnect/expiry/restart.

LSP types/framing remain inside one optional adapter and start with full-sync open/change/save/close only. URI is
a locator, not source authority. Do not create an incremental patch engine or semantic-index platform.

If implementation is selected, change the direct query/LSP consumer and remove the old overlay API in the same
slice. Run minimal Rust 1.98 compiler/Clippy for actual consumers. Self-tests of a disconnected store are not a
product gate.
