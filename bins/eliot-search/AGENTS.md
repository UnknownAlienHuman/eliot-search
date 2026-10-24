# Working on eliot-search

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md),
[local protocol](../../crates/search-provider-protocol/README.md) and local [source map](FUNCTIONS.md).

The CLI owns invocation capture, one owned local connection, current typed request construction, bounded
presentation and exit status. It does not own query/access policy, redb/object/Qdrant state or grants.

Use the smallest supported route first: spawn/own the daemon child and communicate over inherited
stdin/stdout through the one current framed client. A persistent owner-only OS pipe is optional. Do not
require pairing, shared keys, grants, binding records or capability digests for child stdio. A selected
restricted-client profile authenticates separately and terminates into the same current dispatcher.

Capture `args_os()` and config once. Build `Search | Exact | Continue | Expand` directly. Do not translate to
`RecipeIdV1`, marked indexed payloads, tab/hex commands or opaque source handles. `Expand` uses one
`WindowItemRef` and bounded relative context, never arbitrary revision offsets.

Render typed human/JSON/NDJSON output with exact source/revision/location/excerpt and truthful partial/gaps.
Use Serde; do not substring-parse events or concatenate JSON. Convert timeout once to an absolute deadline;
interrupt sends best-effort cancellation for the live request and closes within the remaining bound.

Delete the duplicate legacy core/session/endpoint/harness path with its last callers. Preserve child
ownership/cleanup, non-UTF-8 invocation behavior, finite output and useful exit mapping. After code, run
minimal Rust 1.98 compiler/Clippy for actual CLI/client/protocol/daemon consumers.
