# Agent contract — search-point-identity

Own stateless S11 point identity. Follow [root instructions](../../../AGENTS.md) and the
[execution protocol](../../../docs/product/EXECUTION_PROTOCOL.md) for named owner/consumer cutovers.
An unresolved field/profile belongs to its existing contract owner; do not invent a compatibility rule.

## Existing owner

Use `search_point_identity::s11` and [S11_PROFILE.md](S11_PROFILE.md). They already define the
eight-coordinate canonical key, full BLAKE3 identity, all 128 address bits, independent observed
identity and collision disposition. Use the shared `search-contracts` codec/digest implementation;
this package owns the S11 preimage/profile, not another CBOR or hashing engine.

A UUID is an address. Overwrite requires the full digest and independently reconstructed key,
including role/schema and the exact generation's profile binding. Preserve rejection of foreign
addresses, copied digests with changed coordinates and truncated-address collisions.
Named vectors of one unit under one immutable profile set share one point identity.

Do not derive source identity, perform upserts, maintain an identity registry, or claim collisions
impossible. Vendor types stay private. Keep relevant typed collision/canonical-encoding failures.

## Legacy disposition and evidence

The root exports are a separate legacy profile, not aliases or an S11 conversion API. Planner and
publication already use S11; #262 still ports the bridge. #329 removes obsolete producers/registry/
exports using the actual caller census. Preserve a decoder only for an identified stored format and
reader, with its deletion owner; do not keep a whole producer to manufacture a rejection fixture.

Retain the frozen independent canonical-byte/digest/address vectors, namespace separation and
collision/changed-coordinate cases. The root protocol selects production checks and defers fixture
execution until integrated implementation. Relevant internal dependencies are the accepted manifest;
new boundaries/pins need manager review. Remove duplicate behavior before considering package moves;
historical wave/line targets do not require forwarding wrappers or another crate.
