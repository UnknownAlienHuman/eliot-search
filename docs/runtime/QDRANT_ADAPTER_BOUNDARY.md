# Qdrant adapter boundary

Qdrant is the only indexed backend. Keep its SDK private to one adapter so vendor changes do not spread
through source/query/result code. This is an implementation boundary, not a security principal,
qualification authority or process manager.

## Runtime boundary

- Only `search-qdrant-bridge` depends directly on `qdrant-client`.
- Keep vendor request/response/filter/schema types private and expose only the concrete operations used by
  publication/query.
- Baseline daemon composition supplies a configured endpoint plus optional actual credential. The bridge owns
  one client/runtime for both publication and query.
- Startup observes the current server/version/capabilities and verifies the exact selected physical
  collection generation/schema. A historical probe report or executable hash is not a constructor token.
- Managed child ownership is optional. If selected, a small concrete process owner supplies the endpoint and
  actual child lifetime; it does not change the bridge API or add qualification receipts.
- Keep bounded full/narrow mutation, query and exact readback with one absolute deadline/cancellation.
  Possible mutation effects remain unresolved for publication to observe; reads commit nothing.

Use ordinary Cargo metadata and Rust visibility to keep the SDK local. Compile actual consumers and review the
small application API. There is no need for another import/module/alias/taint analyser.

## Retire the partial source analyser

`xtask/src/qdrant_boundary/source.rs` and `source/**` contain 16 Rust files / 3,293 physical lines at
`c8e38b6`, including fixtures. They tokenize Rust, reconstruct imports/aliases/module graphs and propagate
vendor-type reachability while still relying on compiler/review for macros, conditional modules, shadowing and
other language behavior.

Under #214 remove this analyser and its shape/command coupling. Keep useful Cargo dependency pin/ownership
checks and actual adapter behavior. A public SDK leak is fixed in the adapter API; do not build another Rust
language implementation to certify every spelling.

## Process lifecycle is separate and optional

The current `search-qdrant-supervisor` is about 90.7 KiB of production Rust and has no product caller. It
models artifact qualification, owner/secret/containment receipts and restart/quarantine. Its own source states
that it cannot apply Windows Job Object/ACL APIs and accepts caller-created evidence instead.

Baseline product integration connects to a configured local Qdrant endpoint. Optional managed-child mode may
later own an actual spawned process handle/platform child-tree guard, private config and bounded shutdown. It
must use this bridge for authenticated readiness and cannot block #263/#264. See
[`search-qdrant-supervisor/FUNCTIONS.md`](../../crates/search-index-qdrant/search-qdrant-supervisor/FUNCTIONS.md).

## Qdrant changes and evidence

When changing the vendor release, update the selected dependency/artifact, adapt private translation and run
the connected schema/query/mutation behavior. Release/upgrade evidence records what actually ran; it is not
stored as runtime authority.

Normal startup verifies its current service and selected schema. Compilation, source-pattern matching, a
matching version string or a past qualification report does not qualify a live server. Likewise, missing
release paperwork cannot disable an otherwise observed development endpoint while the product is being
implemented.
