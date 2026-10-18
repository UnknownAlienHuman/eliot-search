# Point identity: current S11 and replacement profile

This file records the implemented v1 profile and the required product cutover. It is not a promise to
preserve pre-release point bytes.

## Current v1 source

`search_point_identity::s11` currently hashes a closed eight-field key:

```text
schema_version
installation_incarnation_id
collection_generation_id
projection_membership_id
representation_id
unit_id
projection_profile_set_id
point_role
```

The full BLAKE3 identity is projected through a separate domain to a 128-bit address. UUID rendering keeps
all address bits. `compare_existing_identity` rejects an occupied address with a different full identity or
key. These are useful mechanics.

The current frozen-vector fixture and canonical CBOR ceilings describe existing code only. Planner and
publication use this module; the bridge still has a separate legacy address/payload model.

## Why v1 must change

The current `unit_id` is derived from full revision provenance, absolute source/line coordinates and
ordinal. `representation_id` is also view-specific. Including both in the Qdrant point key binds retrieval
storage to one source revision and turns a small early insertion into a large create/retire suffix.

The payload and projection manifest repeat the same coupling. A cache can avoid some model execution but
cannot retain the old point or old citation correctly. Wrapping v1 with another mapping layer would preserve
the amplification and add another identity owner.

## Replacement profile

For a selected new physical generation, derive the full retrieval identity from a small closed value:

```text
key schema version
collection generation
projection membership or stable source grouping
occurrence incarnation
retrieval profile / role
exact retrieval-input digest
point role
```

`occurrence_incarnation` distinguishes equal passages within one source and changes when an occurrence is
removed and later reintroduced. It comes from the structure-first source owner; it is not an absolute byte
offset, current revision ID or content hash by itself.

Source revision, representation, structural occurrence and exact range belong to a versioned
`ViewManifestEntry`. Consecutive views may point the same retrieval point to different exact ranges; old
pinned views retain their old binding. Changed retrieval input/profile/incarnation creates a new point.

The selected generation binds the exact key/profile bytes. The full identity and address domains remain
separate, and occupied-address collision refusal remains. No registry is required.

## Cutover

Change together:

- point-identity key/decoder/comparator;
- projection planner and ordered view manifests;
- shared payload fields and Qdrant bridge conversion/readback;
- publication diff/cleanup;
- query point→view-manifest→source lookup.

Use a new derived collection generation and rebuild points. Delete the v1 issuer/decoder/goldens and root
legacy FNV/registry after callers switch. Do not rehash stored IDs in place, mint both profiles or add a
compatibility migration. Unsupported old application roots remain untouched.

## Required behavior

- same occurrence + same exact retrieval input/profile yields the same point within the generation;
- changed input/profile/incarnation yields a different full identity;
- duplicate equal passages have distinct incarnations;
- address collision with another full identity blocks mutation;
- harmless coordinate/revision shifts update only the new view binding;
- old pinned view still opens its old exact evidence.
