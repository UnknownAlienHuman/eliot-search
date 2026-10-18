# search-provider-protocol

Framing, typed request/result encoding and request lifecycle for the CLI and local daemon.
This package performs no socket, pipe, filesystem, database or secret-store I/O.

Use the [implementation guide](../../docs/implementation/README.md#4-simplify-and-connect-the-query)
for the single local exchange and [FUNCTIONS.md](FUNCTIONS.md) for current mechanisms.

## What exists

- [FrameCodec](src/frame.rs) validates length-prefixed JSON; [TypedRecordBuffer](src/frame/transport.rs)
  assembles a bounded record.
- [Client/server codecs](src/frame/client.rs) encode the existing V1 typed messages.
  [Evidence schemas](src/frame/client/evidence_schema.rs) and progressive evidence frames also exist.
- [BoundSession](src/binding.rs), request guards and terminal/cancellation code implement the current
  paired, authenticated session. Pairing/grant transcripts and endpoint descriptors are implemented too.

Existing CLI native-client and daemon provider modules consume parts of this API. The public
[stdio client](../../bins/eliot-search/src/public_client.rs) still uses its own tab/hex exchange and
substring event parsing. The daemon V1 recipe registry does not serve the delivered V2 evidence
requests. A codec or session constructor alone is not a connected search endpoint.

## Refactoring target

Use one bounded typed exchange through owned child stdio, or an owner-only local pipe when needed.
Reuse framing, correlation, cancellation and terminal behavior. Change decoder, dispatch, handler and
result encoding together, removing the replaced tab/hex, loopback and parallel V1/V2 forms.

Use Serde/serde_json in the existing owners for the new wire. Remove mandatory canonical re-encoding,
body MACs, paired hello and grants from trusted-local constructors with their callers. Do not create
fake credentials to pass an old constructor. A selected restricted-client profile retains real
credential verification, replay checks and delegated scope enforcement.

Bound incoming frames before allocation and typed decoding; bound outgoing records, queued work and
in-flight requests. Keep clear malformed/version errors and cleanup on EOF/cancellation. Capability
reporting describes connected handlers. Source truth, access policy and backend operations stay with
their application owners. See [AGENTS.md](AGENTS.md).
