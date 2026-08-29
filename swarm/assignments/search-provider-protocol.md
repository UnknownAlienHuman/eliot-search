# `search-provider-protocol` implementation packet

**Path:** `crates/search-provider-protocol`  
**Capability:** C30 generic edge  
**Delivery:** W1 transport / P01; W8 integration / P14  
**Gate:** W1 shell after W0; full binding after W7 security receipts  
**Trace:** S32-S33, H16, P01, P14

## Mission

Implement length-prefixed local transport, authenticated session/binding state and bounded request flow
without importing client authority or serializing server-side handle records.

## Owns

Frame codec, hello/version negotiation, pairing/binding authentication, replay-safe connection sequence,
in-flight registry, cancellation/deadlines, progress ordering and protocol errors.

## Must not own

A second framing protocol, ACL-only authentication, store/index clients, client admission, unbounded
fragmentation/compression, token interpretation, or serialization of `SearchSourceHandleRecord` /
`ContinuationRecord`.

## Logical operations

1. `encode_frame(envelope, limits) -> Result<Bytes, ProtocolError>`
2. `decode_frame(bytes, limits) -> Result<ProviderEnvelope, ProtocolError>`
3. `negotiate_hello(local, remote) -> Result<NegotiatedProtocol, ProtocolError>`
4. `authenticate_binding(hello, pairing, installation) -> Result<BindingContext, ProtocolError>`
5. `admit_request(connection, envelope) -> Result<RequestGuard, ProtocolError>`
6. `cancel_request(request_id) -> CancelOutcome`
7. `emit_progress_or_result(guard, message) -> Result<(), ProtocolError>`

## Invariants

- frame cap 8 MiB and in-flight cap 32 by default;
- sequence monotonicity and replay rejection;
- idempotent cancel/disconnect cleanup;
- explicit major/minor negotiation;
- no baseline compression/fragment assembly;
- capability descriptor is binding-filtered;
- provider JSON may contain only opaque source/continuation handle tokens, never their server records;
- opaque tokens are redacted from diagnostics.

## Exit evidence

Frame golden, pre-allocation oversize rejection, replay/sequence tests, mutual hello, in-flight limit,
cancel/disconnect cleanup, ACL-not-auth proof, wire-handle schema allowlist, server-record serialization
compile/schema rejection and token-redaction fixture.

Target `src/` ≤7,500 lines; split review before 8,500 total; hard stop at 10,000 including local tests.
