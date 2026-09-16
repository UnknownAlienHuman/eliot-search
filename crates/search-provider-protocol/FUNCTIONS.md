# Function contract — `search-provider-protocol`

**Status:** bounded framing, pairing, binding, body-bound admission, terminal
request lifecycle and standalone-grant request-body kernels are implemented;
complete W8 live integration/qualification remains open.

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

## Standalone grant request body

### `encode_standalone_grant_request(request) -> Result<Vec<u8>, ProtocolError>`

Emits one fixed-order canonical UTF-8 JSON body containing requested scope,
recipe, budget, disclosure/sensitivity, permission and TTL ceilings. Binding,
principal, installation, operation and grant identities are absent by design.

### `decode_standalone_grant_request(bytes) -> Result<StandaloneGrantRequestV1, ProtocolError>`

Rejects non-canonical order/spelling, whitespace, escapes, duplicate or oversized
sets, invalid enum/profile values, zero generations/TTL, permission widening
shape and trailing bytes. Re-encoding must equal the exact input bytes.

These bytes are intended to be bound by an authenticated envelope body digest.
Decoding creates no grant and no authority; daemon composition supplies the
session binding, operation identity and server policy.

## Connection and request lifecycle

### `admit_sequence(connection, sequence) -> Result<(), ProtocolError>`

Sequences are monotonic; duplicate, replayed or regressed values fail closed.

### `admit_request(connection, envelope, limits) -> Result<RequestGuard, ProtocolError>`

Checks authenticated binding, request ID uniqueness, relative deadline and the
32-in-flight ceiling before forwarding a bodyless control request. This
compatibility surface does not prove possession of external body bytes.

### `admit_body_bound(connection, envelope, expected_proof, observed_body_digest, sequence, now) -> Result<RequestGuard, ProtocolError>`

Checks active paired session, exact version and server nonce, keyed envelope
proof and constant-work equality between the envelope body digest and the
digest computed by the adapter from exact request bytes. Every body check,
deadline and capacity check completes before sequence/replay/in-flight mutation.
On success the connection retains the request guard until terminal completion,
cancellation or disconnect.

### `emit_progress(guard, event) -> Result<(), ProtocolError>`

Progress sequence is monotonic and bounded, carries counts/phases/reasons only and never source/query
content. Progress is non-terminal.

### `complete_request(connection, request_id, terminal) -> Result<RequestStatus, ProtocolError>`

Records exactly one terminal result/error/cancelled class and releases exactly
one in-flight slot. The terminal guard remains retained for duplicate-terminal
detection until disconnect. A cancelled request may complete only as cancelled;
it cannot be relabelled success.

### `cancel_request(connection, target_request_id) -> CancelOutcome`

Idempotently marks cancellation and releases the request's in-flight slot.
Unknown or already terminal IDs return bounded non-sensitive outcomes. A
subsequent terminal acknowledgement, when required by the transport, must be
`cancelled`.

### `disconnect(connection) -> DisconnectReceipt`

Cancels all request-local work, releases guards and invalidates connection-scoped state. Handle/
continuation records follow their owning packages and binding policies.

## Configuration and failures

Implements `config/sections/protocol.md`. Security floors—pairing, frame/in-flight ceilings,
no compression and no fragmented assembly—cannot be weakened. Failures include frame too large,
malformed/unknown version, replay, binding auth, body mismatch, too many in flight, deadline,
cancellation and invalid message transition.

## Required fixtures

Frame golden; oversize rejected before body allocation; malformed/unknown tags; major/minor negotiation;
pairing required beyond ACL; canonical grant-body round trip and non-canonical/duplicate/oversize
rejection; body mismatch before mutable admission state; sequence replay; 32-in-flight; guard retained
until terminal release; duplicate terminal rejection; cancelled-to-success relabelling rejected;
progress ordering/content minimization; idempotent cancel; disconnect cleanup; protocol public API has no
store, Qdrant or client-authority path.
