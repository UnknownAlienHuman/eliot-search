# CLI — invocation, one owned session and bounded presentation

[Protocol](../../crates/search-provider-protocol/FUNCTIONS.md) owns framing, clocks and terminal races;
[client](../../crates/search-provider-client/FUNCTIONS.md) owns the serviced session, process and concrete I/O;
[windowing](../../crates/search-query/search-continuation/FUNCTIONS.md) owns paging and item lifetime.
This is the replacement recipe, not a claim that indexed Search currently works.

## Inspected source and removal boundaries

R013/R022 inspected public_client and app/core: separate launch loops, inactive input during a request,
line/string parsing, display before terminal, arbitrary raw-range expansion, child cleanup escape paths and
Command::output followed by a late stdout-only limit. These Rust paths remain until the real session replaces them.
Core error output also replaces quotes rather than correctly serializing JSON. The audit registry retains exact
source revisions and paths; do not repeat those implementations in another wrapper.

R023 at `305ccab1` traced these concrete paths:

| Path | Boundary / decision |
|---|---|
| remote_client → endpoint_client → provider_client → core | Feature-only TCP/token client, not the typed pipe client. Remove with legacy dispatch; do not reconnect the retired server. |
| native_bootstrap → native_registered | Descriptor/key checks followed by a compatibility binding without the installation anchor required by its mandatory grant. Remove, not repair for the local baseline. |
| native_local_registered → provider_client/local.rs | Resolver and private Windows opener are not called by the inspected CLI dispatch. Do not build a registration platform to make unused constructors callable. |
| native_credentials | Provider-pairing credential reader, distinct from retained-revision keys. Remove only its exclusive consumers; protect shared secret owners/#598. |

R024 removes sync-directory and verify-directory-manifests from primary help because the inspected dispatcher
has neither arm. It labels remaining root forwarding as legacy and no longer advertises an installed native
provider. Health/shutdown currently spawn a shell child; they do not inspect/stop an already running daemon.
These are help corrections, not new command routing. The feature-only search/lexical aliases and old core help
remain with their obsolete owners until cutover; no distinct BM25 path follows from their names.

The native/canonical counterpart audit is still partial. Do not delete every native file from this table alone.

## Capture once and use the real session

Capture args_os/settings once and preserve OS paths. Validate help/version and request shape before spawning.
Construct current Search/Exact/Continue/Expand or connected administration directly. No recipes, grants,
registration producer, endpoint discovery, marked query payload or tab/hex conversion precedes child stdio.
A restricted/shared-pipe adapter needs a selected consumer, not an old constructor to preserve.

The CLI supplies executable/settings to the one client session, which immediately owns the child/streams and
cleanup guard. Do not add another wait loop for diagnostics, forwarding or interactive commands. A busy root
reports the conflict, with no hidden second installation or transport fallback. Never kill an independent daemon.

The request deadline covers setup and encoding. A synchronous public invocation uses the serviced client;
input/control/output remain observable during work. Its concrete Tokio/terminal-reader choice belongs to the
client recipe, not duplicated per-command subprocess frameworks.

## One-shot and session

| Mode | Contract |
|---|---|
| One-shot | Bounded terminal cards/gaps, no reusable window/item locators, then owned-child cleanup. |
| Session | Same child/streams across requests. Continue/Expand use that session; terminal ends one request, not the session. |

A new invocation cannot use old process-local locators. One active data request and bounded control suffice;
no general scheduler or completed-request archive.

## Presentation and diagnostics

Decode/validate the whole framed terminal before displaying cards or locators. Render typed human/JSON/NDJSON
with Serde, retaining rank, source/revision/representation/location and truthful gaps. Do not substring-parse,
replace quote characters or mistake a signed status receipt for verification of printed payload/evidence.
Progress is optional and contains no results requiring a later window commit.

Captured stderr is serviced alongside stdout with finite retained diagnostics, outside the result parser.
Inherited/discarded stderr is a separate policy, not bounded capture. Never wait for stdout/exit while leaving a
piped stderr unread. Command::output/wait_with_output plus a later length check is not a bounded collector.
A raw status passthrough is not the selected validated/cancellable exchange. Parent output failure ends the owned
session instead of leaving child work running.

## Cleanup and expansion

All exits use the same client owner: stop new work, best-effort control, close stdin, finite graceful allowance,
termination when necessary and observation/reaping of exit. Issuing kill is not reap or rollback. No indefinite
join on terminal input, no detached per-timeout workers, and no hard OS timing promise without platform evidence.
An ordinary typed failed/cancelled request on a healthy aligned session differs from broken framing; protocol
owns whether the connection remains usable.

Expand uses window_id/item_index and bounded relative context in the emitted representation. It cannot select
arbitrary raw offsets, revive expiry or silently change revisions. Lost sessions repeat Search.

## Cutover and checks

Change actual CLI/client/protocol/daemon handlers together. Remove old parser/launch/endpoint/session owners and
exclusive fixtures; keep OS-path handling and useful causal child/output cases. Minimal pinned compiler/Clippy
must select actual binaries, not just libraries. Later checks cover complete terminal before display, two
requests and Continue/Expand, Ctrl-C/EOF during read/write, diagnostic backpressure/limits, output failure,
cleanup, busy root and independent-invocation locator rejection. No R024 runtime success is claimed.
