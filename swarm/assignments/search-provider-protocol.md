# `search-provider-protocol` implementation packet

**Path:** `crates/search-provider-protocol`  
**Capability:** C30 generic edge  
**Delivery:** W1 transport / P01; W8 integration / P14  
**Gate:** W1 frame/session primitives blocked until W0; full binding/integration blocked until W7 security receipts  
**Trace:** S32-S33, H16, P01, P14  
**Direct public handoffs:** `search-contracts`, `search-domain`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Implement the generic local provider transport, authenticated session/binding state and bounded request flow without importing client authority.

## Owns

- u32-LE length-prefixed UTF-8 JSON frame codec
- hello/version/capability negotiation and replay-safe connection sequence
- pairing/binding authentication and request lifecycle
- in-flight limit, cancellation, deadlines, progress ordering and protocol errors

## Must not own

- a second framing protocol
- named-pipe ACL as sole authentication
- raw store/Qdrant access
- client admission or canonical writes
- unbounded frame fragmentation/compression

## Logical primitives

- FrameLimits, FrameCodec, HelloRequest, HelloResponse, NegotiatedProtocol, ConnectionState, BindingContext, InFlightRegistry, RequestLifecycle, ProgressSequence, ProtocolErrorFrame

## Logical operations

1. `encode_frame(envelope, limits) -> Result<Bytes, ProtocolError>`
2. `decode_frame(bytes, limits) -> Result<ProviderEnvelope, ProtocolError>`
3. `negotiate_hello(local, remote) -> Result<NegotiatedProtocol, ProtocolError>`
4. `authenticate_binding(hello, pairing, installation) -> Result<BindingContext, ProtocolError>`
5. `admit_request(connection, envelope) -> Result<RequestGuard, ProtocolError>`
6. `cancel_request(request_id) -> CancelOutcome`
7. `emit_progress_or_result(guard, message) -> Result<(), ProtocolError>`

## Required invariants

- frame cap 8 MiB and in-flight cap 32 by default
- connection sequence is monotonic with duplicate/replay rejection
- cancel is idempotent and disconnect releases request-local resources
- major mismatch fails; minor negotiation is explicit
- no compression or unbounded fragment assembly in baseline
- capability descriptor is filtered to binding-visible opaque memberships

## Typed failure surface

- `PROTOCOL_VERSION_MISMATCH`
- `FRAME_TOO_LARGE`
- `REPLAY_DETECTED`
- `BINDING_AUTH_FAILED`
- `TOO_MANY_IN_FLIGHT`
- `REQUEST_DEADLINE_EXCEEDED`
- `REQUEST_CANCELLED`

## Exit tests / evidence

- `frame_codec_golden`
- `oversized_frame_rejected_before_allocation`
- `sequence_replay_rejected`
- `mutual_hello_and_major_minor_negotiation`
- `inflight_32_limit`
- `idempotent_cancel_disconnect_cleanup`
- `named_pipe_acl_not_only_auth`

## Suggested internal modules

```text
search-provider-protocol/src/
  frame.rs
  codec.rs
  hello.rs
  version.rs
  binding.rs
  sequence.rs
  inflight.rs
  cancel.rs
  deadline.rs
  message.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 7,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Split frame codec from session only if the crate exceeds the split threshold and both can retain independent test/replacement boundaries without a forwarding facade.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.
