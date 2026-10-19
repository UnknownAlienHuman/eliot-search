# Working on sources

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md) and [source map](README.md).

Complete one path from selected root to immutable bytes and a typed current source head. Preserve #598's
active private work and consume only its accepted handoff.

Do not maintain the current 1.22-million-byte policy/receipt/provenance/records/history/identity/registry
stack. Change actual producers and consumers together and remove the obsolete package/API/fixture/dependency
in the same cutover.

Rules:

- transfer the exact body already acquired to the one object writer; never reread the pathname;
- admission is one small pure pre/post-read decision, not a permanent receipt/archive;
- source identity is current locator plus optional proven physical key; content digest is revision identity;
- typed redb rows own current roots/sources/heads; no full registry replay or operation history;
- `PublishedView` owns immutable query evidence, so registry/path history is not needed for old queries;
- watcher events are hints; only completed inventory/stable reads change current state;
- unsupported old roots fail before writes and remain intact;
- no forwarding crates, v3 compatibility families or generic source service locator.

After production code, run minimal Rust 1.98 compiler/Clippy for changed source/object/control/daemon consumers
and focused stable-read/head/restart/rename cases. Package byte goldens and replay matrices are removed with
their APIs.
