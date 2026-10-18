# Query planning

Use the [implementation guide](../../../docs/implementation/README.md#4-simplify-and-connect-the-query).
This owner turns a resolved scope, pinned published view, actual retrieval profile, query and finite
limits into the operations the query needs. Keep it pure and call it directly.

`compile_canonical_plan` currently requires a caller-created task plan, grant and several fingerprints.
Remove these requirements from trusted-local planning with their consumers. Do not create synthetic
proofs or a second planner to get around the old API. Restricted-client checks belong at their actual
admission boundary.

Reuse the existing lexical encoders. Ordinary sparse retrieval needs encoding, bounded retrieval,
source validation and results; exact scanning is requested separately. No mandatory exact denominator,
proof job or persistent plan is needed for top-k.

Keep useful leg/fusion logic for real selected modes. All legs use the same resolved scope and pinned
view. A request's profile name is not proof that published vectors used that profile.
Vendor/database calls remain in the adapter/composition.

Change immediate request/executor consumers with the planner. There is no automatic contract-change
issue, crate-only edit restriction, line quota or frozen fingerprint compatibility requirement.
Check real ordering, scope/profile consistency, bounds and cancellation; no source-shape test can
substitute for the connected query.
