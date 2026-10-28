# CLI — invocation and bounded presentation

The CLI captures invocation/settings, uses one owned client session and renders typed results. It owns no source
scope, stores, planning or second process manager. This is a target recipe, not a working-product claim.

## Ownership and source seams

Read an adjacent recipe only when its boundary changes:

| Boundary | Owner |
|---|---|
| Frames, deadlines and terminal races | [Protocol](../../crates/search-provider-protocol/FUNCTIONS.md) |
| Child/streams, serviced I/O, cancellation, cleanup | [Client](../../crates/search-provider-client/FUNCTIONS.md) |
| Pinned paging and emitted-item lifetime | [Query window](../../crates/search-query/search-continuation/FUNCTIONS.md) |
| Actual receiving handlers | [Daemon](../eliot-searchd/FUNCTIONS.md) |

At the recorded pre-R055 head, the primary target is src/entry.rs with legacy remote/public_client/app/core routes.
Inspect their current dispatch and real consumers before replacement. Detailed historical route findings and the
five already removed orphan files remain in the
[immutable source audit](https://github.com/UnknownAlienHuman/eliot-search/blob/45ddf887c9e84861b1302334a1f5ecabb263f7f3/bins/eliot-search/FUNCTIONS.md).
They are not additional implementation steps or proof that a current command runs.

## Capture and dispatch

Capture args_os/settings once, preserve native paths, and validate help/version/request shape before spawning.
Create current typed requests directly, not old recipes, grants, endpoint discovery or tab/hex conversions.
The client immediately owns its child/streams/cleanup guard. A busy root fails without a second installation or
transport fallback. Never kill an independent daemon.

The original deadline covers setup and encoding. Input/control/output remain serviced during work; a synchronous
public call may wrap that session but cannot replace it with blocking per-command subprocess loops.

## Modes

One-shot emits a complete bounded terminal response without reusable window/item locators, then cleans its child.
A live session keeps the same child/streams; Continue/Expand use that session. A new invocation cannot reuse old
locators. One active data request plus bounded control state is enough; no completed-request archive.

## Output

Decode and validate the entire terminal frame before displaying cards or locators. Use typed human/JSON/NDJSON
rendering with proper serialization. Preserve final order, source/revision/representation/location and truthful
gaps. Optional progress contains no uncommitted result cards.

Service captured stderr alongside stdout with a finite diagnostic bound. Inherited/discarded stderr is a separate
policy, not bounded capture. Do not wait for exit/stdout with unread piped stderr or use Command::output followed
by a late size check. Output failure ends the owned session instead of leaving child work alive.

## Cleanup and expansion

All exit paths use the client cleanup owner: stop work, bounded best-effort control, close stdin, finite graceful
allowance, terminate when needed, observe/reap exit. Kill is not reap or rollback. No indefinite input-thread join,
detached timeout workers or unproven hard OS timing promise. A healthy typed failure is distinct from broken
framing; the protocol determines session usability.

Expand names an emitted window item and bounded relative context in its representation. It cannot select arbitrary
revision offsets, revive expiry or switch revisions. A lost session repeats Search.

## Cutover and verification

Change CLI/client/protocol/daemon consumers together. Remove duplicate parser/launch/endpoint/session routes and
exclusive fixtures; preserve native-path handling, bounded diagnostics and actual cleanup behavior. Protect shared
revision-key owners and #598 when removing pairing-only code.

After production changes run minimal locked Rust 1.98 check/Clippy selecting actual binaries, not libraries only.
Later execution covers terminal-before-display, live cancellation/EOF, two-request paging/expansion, backpressure,
output failure, cleanup, busy root and independent-invocation locator rejection. Record unrun cases explicitly.
