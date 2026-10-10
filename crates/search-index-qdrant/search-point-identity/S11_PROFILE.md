# S11 point identity profile and consumer handoff

Source/API delivery for #256. Product cutover and live Qdrant qualification remain separate.

## Frozen profile

`search_point_identity::s11` owns profile revision 1 and key schema version 1.
It uses `search-contracts` profile `eliot.cbor.length-first.v1` exclusively.
The exact closed map has eight fields:

| Field | CBOR value |
|---|---|
| schema_version | unsigned integer 1 |
| installation_incarnation_id | 16-byte UUID bytes |
| collection_generation_id | 16-byte UUID bytes |
| projection_membership_id | 16-byte UUID bytes |
| representation_id | 16-byte UUID bytes |
| unit_id | 16-byte UUID bytes |
| projection_profile_set_id | bounded nonempty UTF-8 text |
| point_role | unit, relation or auxiliary text |

Map order follows the sole shared deterministic encoder. Source membership,
paths, ranges, access/scoring state and vector contents are absent.
The immutable profile set determines the required named vectors; adding a required
profile publishes a new generation rather than another identity for the same unit.

Full identity is `BLAKE3(eliot/cbor/point-identity/v1 || NUL || canonical_key)`.
Address bytes are the first 16 bytes of
`BLAKE3(eliot/raw/point-address/v1 || NUL || full_identity_32_bytes)`.
UUID rendering preserves all 128 address bits, including version/variant positions.
A UUID is only an address; it is insufficient evidence for idempotent overwrite.

Frozen ceilings: profile text 256 bytes, key CBOR 1024 bytes, each complete digest
preimage 1088 bytes. The latter includes domain and NUL. Callers may narrow resource
limits but may not widen the frozen profile or change canonical bytes/domains.
Unknown schema/roles, oversize and noncanonical or non-closed key data fail closed.

## Frozen oracle vector

`tests/s11_goldens.rs` fixes UUID bytes `[1;16]` through `[5;16]` in key-field order,
profile `lexical-v1`, role `unit`, schema 1. The independently assembled CBOR is
263 bytes. A standalone direct BLAKE3 oracle (no product derivation/helper calls)
produces full identity
`395a97a9b48700cfef5811d51367bc94e89bb899871e2d3c103afccebc429e92`
and address `df8b4260-1ee4-decf-a59f-6028e5d0cecc`. Complete preimages are 292
and 59 bytes respectively. The fixture file also freezes every canonical byte.
The generation owner must bind this fixture/profile alongside the full domain
and budget contract before accepting S11 points.

## API and readback

`ProjectionPointKey` uses existing typed contract identities. `derive_point_identity`
returns a `PointIdentity` whose private evidence is exposed through `key`,
`full_digest` and `point_id` getters. Stored readback is an independent
`ObservedPointIdentity`: address, full 256-bit digest and all eight key coordinates.
The 20-field indexed payload alone lacks the key schema version and point role;
the consumer must obtain those from the exact generation/manifest contract rather
than inventing a default from a matching address or digest.

`compare_existing_identity` returns `Vacant`, `SameFullIdentity` or `CollisionBlock`.
A foreign address is an explicit error. At the expected address any different digest
or coordinate blocks overwrite, including a copied digest with changed coordinates.
No registry, insertion order, process state, transport or upsert is owned here.

## Generation migration

#259 planner and #262 bridge must use this module directly, preserve the full digest
and independently reconstruct the key from authoritative preparation/readback.
Before publishing, the generation/manifest owner must bind the exact profile revision,
domains, limits and frozen fixture identity. A collection-generation UUID alone does
not prove which point-identity profile was accepted. Existing CollectionSchema does
not yet encode a separate identity-profile descriptor; resolving that binding belongs
to the named consumer/publication cutover, not an implicit S11 compatibility rule.

The unchanged root exports (`PointIdentityKey`, `PointIdentity`, `PointId128`,
`PointIdentityRegistry`, existing derivation/validation helpers and legacy profile)
remain solely for existing callers. No adapter converts them to S11. #329 removes
these producers, registry and exports after #259/#262 and before #264 qualification.
Their canonical-ledger classifications remain pending, with removal owner #329.

## Donor boundary

Inspected #207 immutable source `f0ac8a1470336475f28e3fb30610947c83bbb6d1` supplies
only key/collision/UUID behavior and canonical fixture shape. Its local codec,
direct digest ownership, aliases and registry are not adopted. Its old domain grammar
is incompatible with #237; new digest/address goldens describe this new profile and
claim no persisted-ID parity. Cargo manifests and Cargo.lock are unchanged.
