# Function contract — `search-provider-protocol`

**Status:** W1 transport/session contract with W4 end-to-end qualification; no implementation exists yet.

The protocol is local, versioned, bounded and authenticated. It owns framing/session/request lifecycle,
not client authority, source/index stores or search planning.

## Framing and negotiation

### `encode_frame(envelope, limits) -> Result<BoundedBytes, ProtocolError>`

Emits `u32` little-endian length plus canonical UTF-8 JSON. Baseline has no compression or fragmented
message assembly. The 8 MiB ceiling includes the JSON body.

### `decode_frame(prefix_and_body, limits) -> Result<ProviderEnvelope, ProtocolError>`

Validates length before body allocation, UTF-8/JSON/schema/tag/unknown-field rules and exact protocol
bounds. Oversize/malformed input is rejected without unbounded buffering.

### `negotiate_hello(local, remote) -> Result<NegotiatedProtocol, ProtocolError>`

Major mismatch fails. Minor/extension negotiation is explicit and cannot reinterpret load-bearing
fields. Capability availability grants no authority.

### `authenticate_binding(hello, pairing, installation, transport_peer) -> Result<BindingContext, ProtocolError>`

Requires pairing proof plus installation/incarnation/peer binding. Named-pipe ACL or loopback location
alone is insufficient authentication.

## Connection and request lifecycle

### `admit_sequence(connection, sequence) -> Result<(), ProtocolError>`

Sequences are monotonic; duplicate, replayed or regressed values fail closed.

### `admit_request(connection, envelope, limits) -> Result<RequestGuard, ProtocolError>`

Checks authenticated binding, request ID uniqueness, recipe/grant envelope shape, relative deadline and
32-in-flight ceiling before forwarding to server admission.

### `emit_progress(guard, event) -> Result<(), ProtocolError>`

Progress sequence is monotonic and bounded, carries counts/phases/reasons only and never source/query
content. Progress is non-terminal.

### `emit_terminal(guard, result_or_error) -> Result<(), ProtocolError>`

Exactly one terminal result/error/cancelled event is permitted. Results are already bounded by recipe
contracts; protocol does not truncate or reinterpret coverage.

### `cancel_request(connection, target_request_id) -> CancelOutcome`

Idempotently marks cancellation and propagates it to the request cancellation capability. Unknown or
already terminal IDs return bounded non-sensitive outcomes.

### `disconnect(connection) -> DisconnectReceipt`

Cancels all request-local work, releases guards and invalidates connection-scoped state. Handle/
continuation records follow their owning packages and binding policies.

## Configuration and failures

Implements `config/sections/protocol.md`. Security floors—pairing, frame/in-flight ceilings,
no compression and no fragmented assembly—cannot be weakened. Failures include frame too large,
malformed/unknown version, replay, binding auth, too many in flight, deadline, cancellation and invalid
message transition.

## Required fixtures

Frame golden; oversize rejected before body allocation; malformed/unknown tags; major/minor negotiation;
pairing required beyond ACL; sequence replay; 32-in-flight; duplicate terminal rejection; progress
ordering/content minimization; idempotent cancel; disconnect cleanup; protocol public API has no store,
Qdrant or client-authority path.
