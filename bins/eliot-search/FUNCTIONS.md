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

R068 connects `entry.rs → app::run_main`: one native argv capture and direct command selection, shared local
help/version, borrowed forwarding arguments and no `maybe_run` interception in the CLI. Its five unlinked command
copies are deleted from main, not merely from #610. See the [current dispatch map](../../docs/implementation/CODE_MAP.md#existing-dispatch-to-replace).

This is only the invocation increment. Existing effect/line-protocol clients, output handling and daemon routing
still require their named cutovers. The rest of this file describes that target, not delivered transport behavior.
The [immutable earlier source audit](https://github.com/UnknownAlienHuman/eliot-search/blob/45ddf887c9e84861b1302334a1f5ecabb263f7f3/bins/eliot-search/FUNCTIONS.md)
is historical investigation, not a list of additional APIs to restore.

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

Service captured stderr alongside stdout with a finite diagnostic bound. Exhaustion must not stop draining a
live child's pipe: discard further diagnostic bytes with a bounded truncation indication, or terminate and reap
the owned child. Do not wait for exit/stdout with unread piped stderr or use Command::output followed by a late
size check. Normal interactive mode captures and safely renders diagnostics or discards them; raw inherited
child stderr is not safe presentation. Output failure ends owned work instead of leaving a child alive.

## Untrusted display text

Verified provenance does not make excerpts, paths, labels or child diagnostics safe terminal instructions.
Treat them as data at the final output boundary, including error paths and progress:

- Human output visibly escapes ESC, C0/C1 controls, DEL and Unicode bidi controls. Only the renderer emits
  layout newlines/tabs or fixed styling; multiline excerpts receive trusted line boundaries/prefixes. Source
  text cannot inject cursor movement, terminal commands, hyperlinks or a forged diagnostic line.
- JSON/NDJSON uses a real serializer and preserves decoded field values. Escape terminal-control/bidi characters
  through its string-escaping/formatter seam, not quote replacement or a second JSON encoder. Embedded source
  newlines remain inside the serialized string; NDJSON adds exactly its own record delimiter. Never mix raw
  child stdout/stderr or debug text into the structured result stream.
- Display escaping is not normalization of stored evidence. Keep retained bytes, source ranges, identity and
  expansion locators unchanged; never use an escaped/lossy display label as a filesystem path or locator.
- Bound the rendered/serialized bytes, including escape expansion and diagnostic prefixes, before displaying
  cards. Do not allocate an unbounded escaped copy or silently shorten a verified excerpt to fit. Rendering
  failure closes the owned session; it cannot roll back a daemon cursor already committed after wire output.

Use a small renderer in the existing CLI, not a sanitizer service or a blanket non-ASCII ban. The
[Microsoft terminal reference](https://learn.microsoft.com/en-us/windows/console/console-virtual-terminal-sequences)
documents that output control sequences act on the console; checked 2026-10-29. Source-boundary inspection found
raw line forwarding in `src/public_client.rs::drain_until_terminal` at `46258450`; that is a removal seam,
not a reproduced source-to-terminal exploit or a claim that the replacement renderer exists.

## Cleanup and expansion

All exit paths use the client cleanup owner: stop work, bounded best-effort control, close stdin, finite graceful
allowance, terminate when needed, observe/reap exit. Kill is not reap or rollback. No indefinite input-thread join,
detached timeout workers or unproven hard OS timing promise. A healthy typed failure is distinct from broken
framing; the protocol determines session usability.

Expand names an emitted window item and bounded relative context in its representation. It cannot select arbitrary
revision offsets, revive expiry or switch revisions. A lost session repeats Search.

## Cutover and verification

Change CLI/client/protocol/daemon consumers together. Remove duplicate parser/launch/endpoint/session routes and
exclusive fixtures; preserve native-path handling, bounded diagnostics and actual cleanup behavior. Follow the
[local removal boundary](../../docs/architecture/LOCAL_SCOPE.md#remove-legacy-security-wiring): public key/credential
wrappers may leave with their last consumers. Preserve user bytes/keys, unpublished work and real OS/file helpers,
not an unused secret layer or a planned encrypted-storage integration.

After production changes run minimal locked Rust 1.98 check/Clippy selecting actual binaries, not libraries only.
Later execution covers terminal-before-display, live cancellation/EOF, two-request paging/expansion, backpressure,
output failure, cleanup, busy root and independent-invocation locator rejection. Include control/bidi characters
in excerpts/paths/stderr, newline-bearing fields, escape-expansion limits, stderr after its capture limit and
unchanged evidence coordinates. Record unrun cases explicitly.
