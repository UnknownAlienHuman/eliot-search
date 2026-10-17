# search-source-records

Pure source metadata value/schema and rehydration boundary within C03.
The projection first-bind schema stores the complete C00 `ProjectionMembership`
and exact permanent-member provenance under `SourceMembershipId`. Its state,
operation and key profiles are `eliot.registry-state.v6`,
`eliot.registry-batch.v6` and `eliot.registry-state-key.v3`. The existing
`RegistryStateFamily::Membership` classification maps to C02 Membership storage;
no C02 family or journal schema changes are required. Prior profile bytes and
snapshot digest schemas remain unchanged.

These are observed coordinates, not evidence that an ID was allocated by Root,
a representation was qualified or access/scoring populations were authorized.
The decoder retains the original projection UUID and refuses replacement
transitions. No identity derivation or qualification is performed here.
The initial modules are exact relocations of the existing legacy DIRECT and
root-catalog codecs and bounded root observation/currentness records.
The registry re-exports their existing public paths. No I/O, new store,
source/body authority, profile change or readiness claim is introduced.
