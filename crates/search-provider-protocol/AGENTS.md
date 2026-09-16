# Agent contract — search-provider-protocol

You own only `crates/search-provider-protocol/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S32.0-S32.2, S33, H16, P01, P14.

## Mission

Implement the generic local transport, binding and capability edge shared by CLI and optional client adapters.

## Ownership

- named-pipe framing and version negotiation
- mutual authenticated hello and binding state
- sequence/replay/cancellation/flow control
- capability descriptor projection
- request/result/progress envelope lifecycle
- canonical bounded standalone-grant request bodies containing requested ceilings only

## Forbidden ownership

- Qdrant/redb/CAS access
- binding/access-policy persistence or grant authority
- client canonical writes or authority
- raw vendor plans/filters/point IDs
- compression or unbounded fragmentation in baseline

## Allowed dependencies

`search-contracts`, `search-domain`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `FrameCodec::decode(bytes, limits) -> Result<ProviderEnvelope, ProtocolError>`
- `FrameCodec::encode(envelope, limits) -> Result<Bytes, ProtocolError>`
- `BindingSession::accept_hello(hello, peer) -> Result<BoundSession, ProtocolError>`
- `BoundSession::admit(envelope) -> Result<AdmittedRequest, ProtocolError>`
- `BoundSession::cancel(request_id) -> CancelOutcome`
- `encode_standalone_grant_request(request) -> Result<Vec<u8>, ProtocolError>`
- `decode_standalone_grant_request(bytes) -> Result<StandaloneGrantRequestV1, ProtocolError>`
- `project_capability_descriptor(snapshot, binding) -> SearchProviderCapabilityDescriptor`

The grant request body never carries binding, principal, installation,
operation or issued-grant identity. It becomes meaningful only after an
authenticated session and server-side authority composition.

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `PROTOCOL_VERSION_MISMATCH`, `FRAME_TOO_LARGE`, `REPLAY_DETECTED`, `AUTH_FAILED`, `RESOURCE_EXHAUSTED`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `u32-LE framing and 8 MiB cap`
- `canonical standalone-grant body round trip; alternate encoding/duplicates/oversize rejected`
- `maximum 32 in-flight requests and bounded queues`
- `monotonic sequence/replay rejection`
- `idempotent cancellation releases request resources`
- `capability descriptor exposes only binding-visible opaque memberships`
- `named-pipe ACL alone is insufficient authentication`
- `transport primitives can be qualified before client-specific adapter integration`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W1 / P01 transport; W8 / P14 integration**
- Soft `src/` target: **8,500 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.
