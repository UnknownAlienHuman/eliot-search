# Functions — search-provider-protocol

The [implementation guide](../../docs/implementation/README.md#4-simplify-and-connect-the-query)
defines the trusted-local target. The APIs below already exist; their paired/canonical prerequisites
describe current source, not requirements to preserve in every local connection.

## Existing mechanisms

| Source/API | Current behavior and integration boundary |
|---|---|
| [frame.rs](src/frame.rs): `FrameCodec::{encode, decode, validate_payload}` | Validates complete UTF-8 JSON and a `u32` little-endian length prefix. Returns payload bytes, not a typed authorized request. |
| [frame/transport.rs](src/frame/transport.rs): `TypedTransportProfileV1::frame_length`, `TypedRecordBuffer` | Validates the declared length and collects one finite record. Its current `finish` also returns a proof digest; that digest is not required by the new trusted-local wire. |
| [frame/client.rs](src/frame/client.rs): `ClientEnvelopeCodec`, `ServerEnvelopeCodec` | Current V1 typed message encoding/decoding. Replace incompatible local request/result forms with their real consumers. |
| [evidence_schema.rs](src/frame/client/evidence_schema.rs): `encode_evidence_request`, `decode_evidence_request`, `encode_evidence_result`, `decode_evidence_result` | Delivered V2 retrieval/orientation schemas. Decoding validates shape; scope resolution, planning and source validation are elsewhere. The daemon registry currently dispatches V1. |
| [negotiation.rs](src/negotiation.rs): `negotiate_hello` | Chooses a supported version. Version agreement does not imply a connected handler or source permission. |
| [binding.rs](src/binding.rs): `authenticate_binding`, `BoundSession` | Current pairing-dependent binding/session composition. It must change with local callers; a constructor cannot be bypassed with fabricated ceremony material. |
| [request.rs](src/request.rs): `RequestGuard`, `InFlightRegistry`; [binding/lifecycle.rs](src/binding/lifecycle.rs) | Bounded request identity, deadline, progress, cancellation and terminal state. Reuse useful lifecycle behavior without making request history a permanent ledger. |
| [pairing.rs](src/pairing.rs), [grant.rs](src/grant.rs) and request envelopes | Existing ceremony and authenticated-request mechanics. Secret-owning adapters supply real proof material; daemon policy determines actual grants. These are relevant to selected restricted clients. |

The current maximum complete frame is **8 MiB including its four-byte prefix**; body capacity is smaller
by that prefix. Current in-flight capacity is capped at 32. See [ProtocolLimits](src/config.rs).
There is no compressed or fragmented-message assembly. These are finite resource bounds, not claims
that the new local protocol is already integrated.

## Required local exchange

Read the prefix and reject an oversized record before allocating/collecting its body. Decode a complete
bounded body with typed serialization; `FrameCodec::decode` receives a complete buffer and cannot
protect an adapter that already read an unlimited stream. On a live connection, use a bounded frame
with `serde_json::from_slice`, not EOF-waiting `from_reader`. Bound serialization and output buffering too.

Remove duplicate manual JSON grammars, fixed-field-order/re-encode checks and substring event parsing
with the actual CLI and daemon consumers. Keep a clear current format and unsupported-version errors;
old byte-for-byte fixtures do not require compatibility shims or another schema generation.

Correlate each response to its request. Admit bounded work, propagate one deadline/cancellation signal,
and complete each request once. Progress is non-terminal; terminal completion or disconnect releases
request resources. A cancellation does not prove that an external mutation was rolled back.
Return useful bounded result bodies; ordinary diagnostics exclude credentials and source/query bodies.

The trusted-local connection is established by owned stdio or an OS-owned local endpoint. Remove
mandatory pairing/MAC/grant arguments from that API. Restricted-client credentials, replay checks and
scope limits remain real requirements when that profile is selected; framing alone grants no delegated
authority. No store, Qdrant client or access-policy persistence belongs in this package.

## Behavior to check

Through actual client/daemon callers, check prefix/size rejection, partial reads and EOF, typed event
decoding, request correlation, finite queues/output, cancellation and single terminal cleanup.
For selected restricted clients, also check credential/body mismatch, replay and scope rejection.
Malformed data and absent handlers must produce clear errors. Wave labels, source-line quotas and
canonical golden bytes are not runtime or delivery gates.
