# eliot-search — CLI cutover status

The standalone CLI owns invocation capture, one local child/session, current typed requests, bounded presentation
and exit status. It does not own source scope, redb/object/Qdrant state, grants or query planning.

Read [AGENTS.md](AGENTS.md), [FUNCTIONS.md](FUNCTIONS.md), the
[protocol owner](../../crates/search-provider-protocol/FUNCTIONS.md),
[client owner](../../crates/search-provider-client/FUNCTIONS.md) and
[#332](https://github.com/UnknownAlienHuman/eliot-search/issues/332). #264 owns the complete retained-source →
Qdrant → pinned-evidence → CLI result.

## Revision-specific evidence

R033 inspected package/source seams and found multiple incompatible routes: shell scan forwarding, disabled legacy
provider routes, an old provider dependency stack, newline/tab/hex DIRECT commands, opaque handles and shadowed
daemon interception. No CLI/daemon Rust was changed or executed in that pass. These are source observations at
that revision, not permanent current-branch facts or runtime results.

Before deleting or replacing routes, repeat an exact current search across manifests, Rust/non-Rust callers,
constructors, command dispatch/help, features, targets, tests and workflows. Move every surviving client/daemon
consumer in the same cut. Help text, compiled dependencies and hard-coded status fields are not capability
evidence.

## Target

```text
eliot-search
→ capture args_os and selected settings once
→ help/version without spawning
→ Search | Exact | Continue | Expand | Health | Status
→ spawn and own one eliot-searchd child/session
→ one bounded typed framed exchange
→ current daemon dispatcher
→ source-backed typed response
```

Trusted owned-child stdio needs no pairing key, grant, per-frame MAC, installation binding or endpoint/token
file. A selected restricted client authenticates separately and enters the same dispatcher. A persistent
owner-only OS pipe is optional and has its own connection ownership.

The CLI never opens redb/object/Qdrant state, mints source scope or accepts raw backend filters/point IDs.
Continue uses one session-owned window; Expand uses one emitted item locator and bounded relative context. Human
and JSON/NDJSON rendering preserves final order, source/revision/representation/location, verified excerpt and
truthful gaps.

## Removal boundary

Delete old line/tab/hex, endpoint, pairing/grant, native-registration, duplicate launch/session and legacy core
routes with their actual callers. Keep OS-native argument handling, one owned-child cleanup path, active
cancellation, finite input/diagnostic/output limits and useful exit mapping.

A one-shot result returns no reusable locator. A live session keeps the same child for Continue/Expand. Decode
and validate the complete terminal frame before displaying cards or committing window progress. Partial output
fails the session and advances no cursor.

## Completion and evidence

The default CLI reaches one current daemon handler and returns source-backed typed output; cancellation and child
cleanup work under the original deadline. After code, run minimal Rust 1.98 check/Clippy for actual CLI/client/
protocol/daemon consumers, then focused one-shot/session/cancellation/output/cleanup cases. No current runtime
success is claimed by this documentation.
