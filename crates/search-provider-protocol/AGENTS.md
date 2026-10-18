# Local request and result protocol

Use the [architecture](../../docs/architecture/README.md) and
[implementation guide](../../docs/implementation/README.md#4-simplify-and-connect-the-query).

Own one framed typed exchange used by the actual CLI and daemon. Reuse useful framing, correlation,
cancellation and terminal-result behavior. Do not build another transport/session framework.

The trusted-local baseline uses owned child stdio or a local pipe enforcing the owning OS account.
Remove mandatory paired hello, grants, body MACs, canonical re-encode proofs and replay transcripts from
that API with its callers. Do not manufacture valid-looking credentials for old constructors.
A selected restricted-client profile must enforce real credentials and delegated scope separately.

Use existing request/result types with Serde/serde_json where the new wire needs them. Add direct
dependencies to existing owners; do not add duplicate DTO crates. Bound and read one frame before
`from_slice`; do not use EOF-waiting `from_reader` on a live connection. Bound serialized output too.

Replace manual grammar/escaping/field-order code and the CLI's substring event parsing together.
One current format replaces old V1/V2 and loopback forms; old exact-byte fixtures are not a compatibility
promise. Update decoder, dispatch, handler and result encoder in a coherent change.

Keep request correlation, bounded queues, cancellation and clear protocol/version errors where actually
used. EOF/cancellation releases local work; a possible server mutation does not become rolled back.
Capabilities describe connected handlers, not qualification records.

No Qdrant/redb/source-storage or access-policy owner belongs here. Test real framing, event decoding,
size bounds and lifecycle behavior through the application. No wave, package-size quota, formal
contract-change issue or all-profile handoff blocks the actual client integration.
