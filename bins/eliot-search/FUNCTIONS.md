# CLI — invocation and bounded presentation

The current CLI is a one-shot root operator. It captures native arguments, selects an owned daemon child and
validates the child's root response before output. It owns no source scope, stores or query state.
The framed-query requirements below are future work, not current operator interfaces or qualification claims.

## Ownership and source seams

| Boundary | Current owner or future recipe |
|---|---|
| CLI parse, child selection/capture and root response | [src/app.rs](src/app.rs), entered through [src/entry.rs](src/entry.rs) |
| Receiving root commands and consumed settings | [Daemon](../eliot-searchd/FUNCTIONS.md) |
| Root lifetime and control format | [Root](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md), [Control](../../crates/search-control-redb/FUNCTIONS.md) |
| Future frames and terminal races | [Protocol](../../crates/search-provider-protocol/FUNCTIONS.md) |
| Future serviced query session and cleanup | [Client](../../crates/search-provider-client/FUNCTIONS.md) |
| Future query windows and emitted-item lifetime | [Query window](../../crates/search-query/search-continuation/FUNCTIONS.md) |

See the [current dispatch map](../../docs/implementation/CODE_MAP.md#existing-dispatch-to-replace).
Deleted DIRECT, line-protocol, endpoint and harness implementations are not APIs to restore.

## Capture and dispatch

Current syntax:

    eliot-search --help
    eliot-search --version
    eliot-search inspect-data-root [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]
    eliot-search initialize-data-root [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]
    eliot-search check-existing-root-owner [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]
    eliot-search discard-incomplete-data-root [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]

The CLI operation comes first. A positional ROOT and --data-root are alternatives, not two overrides.
Initialization has no external initialization ID. Missing CLI roots may inherit the daemon's consumed
file/environment layer; see the [daemon settings rules](../eliot-searchd/FUNCTIONS.md#current-entrypoints-and-removal-seams).
No-argument invocation and standalone help/version return locally without selecting or spawning a daemon.

Daemon selection is --daemon PATH, then ELIOT_SEARCHD_BIN, then a sibling eliot-searchd executable when present,
otherwise the executable name resolved through PATH. Native path values remain OsString/PathBuf.
The CLI translates the selected operation to the daemon's matching --operation flag and forwards settings
without reading configuration or opening the root itself. It never kills an independently started daemon.

## Modes

Current root commands use one child per invocation and return one root snapshot or failure.
There is no persistent service or reusable query/item locator in this surface.

Future one-shot search must use the final owned-child framed client. A future live session keeps the same
child/streams for Continue/Expand; another invocation cannot reuse its locators. Add those modes with their
real producer/consumer, not a temporary transport or compatibility route.

## Output

The root operator accepts a response matching the requested operation and a closed root-state vocabulary,
or a bounded daemon error code. It checks child status and output bounds before emitting the response.
Its response is factual root state, not source readiness, Qdrant connectivity or search availability.

Future query output must validate the complete terminal frame before displaying cards or locators.
Human/JSON/NDJSON rendering must preserve final order, provenance, exact coordinates and truthful gaps.
Progress must not expose uncommitted result cards. Service stderr alongside stdout with finite capture;
capture exhaustion must not leave a live pipe undrained. Output failure ends owned work.

## Untrusted display text

Current root responses contain closed operation/state/error values, not arbitrary source excerpts.
Future source, path, label and diagnostic rendering must treat content as data:

- Human output visibly escapes ESC, C0/C1 controls, DEL and Unicode bidi controls. Only the renderer supplies
  layout and styling; source text cannot inject cursor movement, hyperlinks or forged diagnostic lines.
- JSON/NDJSON uses a real serializer and preserves decoded field values. Embedded newlines remain inside
  strings; NDJSON adds its own record delimiter. Never mix raw child diagnostics into structured results.
- Display escaping must not change retained bytes, coordinates or filesystem locators.
- Bound serialized bytes including escape expansion. Do not silently shorten a verified excerpt to fit.

Use a small renderer with the consuming query path. The
[terminal reference](https://learn.microsoft.com/en-us/windows/console/console-virtual-terminal-sequences)
explains why terminal controls need treatment; it does not establish an exploit in the current root operator.

## Cleanup and expansion

Current child completion, output-pipe lifetime, timeout and cleanup behavior require final native verification.
Do not infer a strict OS timing bound or installed baseline from a helper probe or source-level timeout constant.

For future framed sessions, all exits close or terminate and reap the owned child. Kill is not reap or rollback.
Avoid indefinite joins, detached timeout workers and unread piped stderr. A typed terminal failure is distinct
from broken framing. Expand must name an emitted item and bounded relative context, preserve its revision,
and reject expiry or independent-invocation reuse.

## Cutover and verification

Keep the root operator route direct while adding the next connected source/query path. Follow
[local scope](../../docs/architecture/LOCAL_SCOPE.md#remove-legacy-security-wiring); preserve user data and
unpublished work, not obsolete command implementations.

Run minimal locked Rust 1.98 check/Clippy for the actual binaries after production changes. Record final native
root/child cases separately. Root operations do not qualify ingestion, publication, queries, live windows or
installation. Source-backed query and presentation cases belong to their later consuming stage.
