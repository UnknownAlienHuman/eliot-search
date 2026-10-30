# Working on sources

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md) and [source map](README.md).

Complete one path from selected root to immutable retained bytes and a typed current source head. Preserve
user data and #598's private work. They are not a reason to wait for a crypto handoff; use
[the selected plain path](../../docs/architecture/LOCAL_SCOPE.md).

## Historical-size boundary

The **1.22-million-byte** policy/receipt/provenance/records/history/identity/registry figure belongs to original
baseline `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`. It is not a current recount, a deletion target or evidence
that every package remains unchanged.

Before removing or merging source packages, inspect exact current manifests, Rust/non-Rust callers, feature
wiring, tests and workflows. Keep useful behavior with its real producer/consumer and remove only displaced
APIs, fixtures and dependency edges.

## Connected-path rules

- transfer the exact body already acquired to the one object writer; never reread the mutable pathname;
- admission is one small pre/post-read decision, not a permanent receipt/archive;
- source identity is a persisted current-source identity plus locator and optional proven physical key;
  content digest identifies a revision, not the source;
- typed redb rows own current roots/sources/heads; no full registry replay or completed-operation history;
- a pinned `PublishedView` owns immutable query evidence, so old queries do not need mutable path history;
- watcher events are hints; only complete inventory/stable reads change current state;
- unsupported or pre-release roots fail before writes and remain intact;
- no forwarding crates, compatibility families or generic source service locator.

After production changes, run minimal Rust 1.98 compiler/Clippy for changed source/object/control/daemon
consumers. Later connected-product cases cover stable-read/head/restart/rename/ABA behavior. Package byte goldens
and replay matrices leave with the APIs they protected.
