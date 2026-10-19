# Local protocol — source audit and replacement map

The [query cutover](../search-query/README.md) and [daemon map](../../bins/eliot-searchd/FUNCTIONS.md) own the
application behavior. This package should contain only framing/current wire DTOs after the cutover.

## Current source inventory

Known production layers before the active CLI reaches a daemon handler:

| Layer | Current behavior |
|---|---|
| `search-provider-protocol` | The first visible source-tree segment already exceeds 236 KiB. It combines framing, manual JSON, pairing/MAC, bindings, grants, replay, recipes, evidence compatibility, cancellation and session lifecycle. |
| `search-provider-client` | About 70.8 KiB of production Rust. Reuses pairing/profile/grant/sequence state and mandatory trusted comparison coordinates before a request can be sent. |
| daemon `provider_composition` | About 136.5 KiB of production Rust. Owns canonical TCP, pairing, capability, serving, router and terminal state and is coupled to access/query grant composition. |
| daemon `public_runtime_service` | About 59 KiB of production Rust. A separate tab-line dispatcher/session/query/handle/continuation path. |
| CLI `provider_client` | Exposes typed-looking `UnsignedRequest`, but maps `IndexedQuery` to a marked payload and then to legacy `op\tquery\t<hex>` through `core::ProviderSession`. |
| `search-contracts` | About 486 KiB of shared source. Carries eleven recipes, authority/plan/lifecycle/proof records and several wire/result families in addition to useful IDs/scope/results. |

This is already well over half a megabyte of production source before CLI legacy core/local modules and the
remaining protocol source are counted. The issue is not one missing adapter; it is multiple parallel
applications.

## Useful current mechanisms

Keep or move:

- four-byte little-endian length prefix;
- reject declared oversize before allocating/reading a body;
- one-frame incremental read buffer;
- finite frame/body/request/output ceilings;
- request-ID correlation and idempotent cancellation of live work;
- optional monotonic progress and exactly one terminal response;
- fail-stop connection behavior after partial output;
- check the original absolute deadline before each read/write/flush;
- output-side transaction callback used by `QueryWindow` to commit only after a complete frame.

The daemon `public_runtime_service::SessionOutput` correctly distinguishes no output from partial output and
refuses to continue the session after a partial logical/I/O failure. `output_deadline` correctly prevents a
new write/flush after the original deadline. Move these small behaviors to the single framed session; do not
retain the line dispatcher around them.

## Current mechanisms to delete

| Source | Replacement |
|---|---|
| `frame/client/wire.rs` fixed-order JSON/base64 encoder/decoder | Serde on bounded current DTOs. |
| `frame/transport.rs` pairing/MAC preface and 32-byte trailer | Plain bounded frame on inherited/owner-only local transport. |
| pairing/challenge/proof/session key modules | None for trusted local baseline; optional restricted adapter terminates before dispatcher. |
| binding commit/revoke/drain/provider lifecycle | Concrete restricted-session owner only when enabled. |
| grant/envelope codecs and capability descriptor | Daemon-resolved scope; optional session subset. |
| V1 recipe/V2 evidence client schemas | One current request/result family. |
| protocol replay/sequence authority | Live request map/correlation only; no durable request history. |
| provider composition canonical TCP/host/router | One daemon local connection/session dispatcher. |
| public runtime tab-line/hex parser | Typed frame decoder and enum dispatch. |
| CLI `UnsignedRequest` → legacy core conversion | Direct typed client request. |

## Target DTO ownership

Contracts own only data used by both ends. Protocol owns the frame envelope and serialization. Client owns
I/O. Daemon owns handler dispatch and state.

```text
// shared current contract
QueryOperation = Search | Exact | Continue | Expand | optional Orient | Compare
OperationResponse = Search | Exact | Continue | Expand | optional Orient | Compare
EvidenceCard / gap / coverage / WindowId / WindowItemRef

// protocol only
ClientMessage { version, request_id, timeout_ms?, operation/admin }
ServerMessage { version, request_id, Progress | Terminal }
ProtocolFailure { stable code, retryable/live-window/partial metadata only when useful }
```

Do not expose Qdrant routes/IDs, redb keys, retained-object internal paths, grant/checkpoint digests, plan IDs
or vendor errors.

### Timeout

A wire request may provide a bounded duration ceiling. On admission:

```text
local_deadline = Instant::now() + min(request_timeout, server_limit)
```

Only the local `Instant` enters `RequestContext`. No cross-process monotonic timestamp is serialized. Every
nested operation uses the remaining time from that same deadline.

### Framed I/O

```text
read_exact 4-byte prefix incrementally
→ validate 1..=max_body_bytes
→ allocate exactly the bounded body
→ read body incrementally under deadline/cancellation
→ serde_json::from_slice::<ClientMessage>
→ validate typed field/collection bounds
→ dispatch
```

Output:

```text
serde_json::to_vec(ServerMessage) under encoded ceiling
→ prefix
→ write_all prefix+body and flush under original deadline
→ commit prepared page/window only after success
```

A partial output is fatal for the connection. Do not send a second terminal/error frame.

## Request lifecycle

The baseline may process one request at a time. If bounded multiplexing is retained:

- reject duplicate live request IDs;
- cap in-flight count and queued progress bytes;
- cancellation targets only a live request on that connection;
- terminal removes the live entry;
- disconnect cancels all live entries and drops its restricted-session/windows;
- no sequence ledger survives connection close or restart.

Health/version/status are direct read operations. Shutdown is available only when the connection owns the
child/daemon lifecycle. Ingest/admin mutations may use current typed commands later; they do not justify
keeping the query recipe/grant platform.

## Contract migration order

1. Define current `Search/Exact/Continue/Expand` request/result DTOs and stable failure codes.
2. Implement one Serde frame codec and one simple synchronous client/session.
3. Add one daemon dispatcher over actual constructed handlers.
4. Connect CLI child stdio to the typed client.
5. Connect search to real Qdrant/evidence/window path.
6. Change output/presentation consumers.
7. Delete old recipe/grant/pairing/manual-codec/provider-composition/line-session paths and their fixtures.

Do not keep a bridge from current DTOs to `RecipeIdV1`, `UnsignedRequest`, `CanonicalRecipeTask` or legacy
`op` lines. Both ends change in one cutover.

## Verification

Focused behavior after code:

- oversize prefix refused before body allocation;
- partial prefix/body reads converge without over-read;
- malformed/unknown version/operation/field bounds fail before handler work;
- one request maps to the correct direct handler;
- duplicate/live cancellation behavior is bounded;
- original deadline does not renew across framing/query/evidence/output;
- partial output closes the connection and commits no window/cursor progress;
- exactly one terminal response per admitted request;
- child EOF/shutdown releases live work and owned process handles;
- no legacy tab/hex, pairing, grant, capability or handle token is constructed.

Run minimal Rust 1.98 compiler/Clippy for changed protocol/client/CLI/daemon/current-contract consumers after
code. Old wire goldens and all-profile pairing/grant matrices are deleted with their APIs rather than gates.
