# search-provider-protocol

One bounded typed local exchange between the actual CLI and daemon. This package owns framing and wire
records; it does not own query planning, grants, capabilities, source access, windows or application state.

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md), [query cutover](../search-query/README.md) and
[source map](FUNCTIONS.md).

## Current source is several protocols at once

The current package combines:

- finite length-prefixed JSON framing;
- a hand-written fixed-order JSON/base64 grammar;
- pairing challenge/proof and per-record BLAKE3 MAC profiles;
- binding commit/revocation/drain state machines;
- authenticated request envelopes and replay ledgers;
- standalone grant request/response codecs;
- eleven V1 recipe schemas plus parallel V2 evidence/orientation schemas;
- session/progress/cancel/terminal lifecycle state.

The visible first portion of `src/` already exceeds 236 KiB of production Rust before several later
pairing/request/session/terminal modules. `search-provider-client`, daemon provider composition and the
separate public runtime service add hundreds of KiB more. Despite this, the CLI's active facade still maps
`IndexedQuery` back into a legacy tab/hex line:

```text
op\tquery\t<hex(strict-prefix + query)>
```

Do not connect these layers to one another. Replace them with one current framed dispatcher.

## One wire

```text
[u32 little-endian body length][UTF-8 typed JSON body]
```

The declared body length is checked against the configured ceiling before allocation/read. One complete
frame is decoded with typed Serde. There is no EOF-delimited JSON, line/tab/hex command grammar, message
fragment protocol, body compression or canonical reserialization requirement.

Partial socket/pipe reads of one frame are normal. Retain the useful prefix-first bounded buffer shape, but
remove its fixed pairing proof trailer. The transport owner supplies one connection and one absolute I/O
context.

## Current request/result family

```text
ClientMessage {
  protocol_version
  request_id
  timeout_ms?          // converted once to daemon-local Instant at admission
  operation
}

operation:
  Search(SearchRequest)
  Exact(ExactRequest)
  Continue(ContinueRequest)
  Expand(ExpandRequest)
  Status
  Health
  Version
  Shutdown             // only on a connection allowed to own daemon lifetime
  Cancel { target_request_id }
  Orient / Compare     // only when real handlers exist

ServerMessage {
  protocol_version
  request_id
  event
}

event:
  Progress(BoundedProgress)   // optional and finite
  Terminal(Result<OperationResponse, ProtocolFailure>)
```

Exactly one terminal event exists per admitted request. Unknown operation/version fails clearly. Optional
operations are present only when a concrete handler is constructed; a capability descriptor is never
request authority.

`Search`, `Exact`, `Continue` and `Expand` use the current contract family. Do not wrap the eleven V1 recipes,
V2 evidence endpoint or legacy DIRECT line into a V3 envelope. Change both real ends together.

## Connection profiles

### Trusted local baseline

Use inherited child stdin/stdout first, or one owner-only local OS pipe when persistence is needed. Ownership
comes from the actual parent/handles or OS ACL/peer identity. The baseline does not need:

- pairing challenges or a shared application key;
- per-frame MACs;
- standalone grants;
- capability digests;
- durable binding records or revocation receipts;
- sequence/replay proof beyond ordinary request-ID correlation on that live connection.

A same-user local process is not granted arbitrary source scope by the transport. The daemon still resolves
the explicit requested scope against the pinned `PublishedView`.

### Optional restricted client

A selected restricted-client adapter may authenticate a real external client, impose an allowed source/view
subset and own expiry/revocation. It terminates into the same typed dispatcher after authentication. Its
credential/binding protocol is separate from the baseline frame and must not force every local CLI request
through pairing/grant ceremony.

## Session semantics

Baseline may start sequential with one in-flight request; bounded multiplexing is optional only when a real
client needs it. Request IDs correlate progress, terminal and cancellation. The daemon converts a finite
wire timeout once into a local monotonic deadline; nested query/Qdrant/source/output work cannot renew it.

A complete response frame is encoded and size-checked before the first byte is written. If a transport write
starts and later fails, the connection is fail-stop: do not append another error frame or consume another
request. Query-window insertion/cursor advancement commits only after the complete frame flushes.

Reads have no durable recovery state. A request disconnect/cancel drops ephemeral work and view/window
ownership. Publication mutations have their own durable pending artifact and are not transported as generic
request receipts.

## Serialization and limits

Use direct Serde derives for current DTOs. Bound before decode/encode:

- frame/body bytes;
- JSON nesting/string/collection lengths through typed validation;
- query/source-selection/page/expansion fields;
- in-flight requests and queued progress;
- terminal/output bytes.

No manual fixed-field JSON parser, custom base64 for ordinary DTOs or canonical byte-golden contract is
needed. Canonical bytes/hashes remain only for actual content/artifact identity.

## Remove after cutover

- `pairing`, `pairing_wire`, authenticated transport profile and 32-byte per-record proof;
- durable binding lifecycle/provider/drain/revocation state;
- request proof/replay ledger and standalone grant codecs;
- V1 recipe and V2 parallel evidence/orientation wire families;
- hand-written fixed-order JSON/base64 grammar;
- capability descriptor and digest exchange;
- protocol-owned handle/continuation/security cleanup;
- legacy line/tab/hex indexed prefix and TCP/token-file endpoint path;
- source/module/golden-byte fixtures whose only purpose is preserving those APIs.

Retain finite prefix framing, partial-read buffering, typed current DTOs, request correlation, bounded optional
progress, cancellation and one terminal event. If the remaining code is small, keep it one package rather
than recreating protocol/client/transport layers.

## Completion

The real CLI sends one typed `Search` frame over owned child stdio, the daemon dispatches directly to the
connected query handler and returns bounded source-backed cards. `Continue` and `Expand` operate on one
`QueryWindow`; partial output advances nothing and closes the connection. No tab/hex line, recipe, grant,
pairing proof, capability digest or opaque source handle is constructed on that path.
