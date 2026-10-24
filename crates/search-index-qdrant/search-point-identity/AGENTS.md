# Working on search-point-identity

Read the root [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and the local
[current/target identity recipe](FUNCTIONS.md).

This package owns only deterministic retrieval-point identity and its compact Qdrant address. It does not
own source revisions, citation ranges, view manifests, registries, Qdrant I/O or compatibility migration.

The current `s11` implementation is delivered source, not a frozen product contract. Its key includes
`representation_id` and revision-bound `unit_id`; keeping those coordinates would recreate every later
point after harmless source-coordinate changes. Change the key with planner, payload, publication, bridge
and query consumers in one cutover.

Keep the useful mechanics:

- one closed typed preimage for the selected current generation;
- full-width content identity before the 128-bit address projection;
- all 128 address bits preserved;
- explicit collision handling when an occupied address has different full identity;
- bounded deterministic encoding through the existing shared digest/serialization owner.

The replacement key identifies stable retrieval behavior: generation, projection membership or stable
source grouping, occurrence incarnation, exact retrieval-input/profile/role and point role. Source revision,
representation and byte/line ranges belong to the versioned published-view manifest.

Do not retain the eight-field v1 profile, goldens or decoder for compatibility. A new internal generation
may use new bytes and rebuild derived points. Remove the root legacy FNV/registry API and the obsolete S11
profile together with their last callers; do not add adapters, dual minting or a collision database.

Verify same input/same occurrence stability, changed input/new point, duplicate equal occurrences,
address collision refusal and old/new pinned evidence through real planner/query consumers. Package-only
golden bytes do not prove useful incremental behavior.
