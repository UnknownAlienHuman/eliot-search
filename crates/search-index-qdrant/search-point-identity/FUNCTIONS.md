# Point identity — one stable retrieval identity

This package derives a stable retrieval identity and its Qdrant address. It owns no registry, source
lifecycle, publication state or vendor I/O.

## Existing source

The current `s11` implementation hashes an eight-field key containing collection generation, projection
membership, representation ID, revision-bound Unit ID, profile set and role. It then projects the full
BLAKE3 digest into a 128-bit address and compares the full identity on readback.

Keep the useful mechanics:

- one closed, versioned canonical key;
- a full 256-bit identity distinct from the 128-bit Qdrant address;
- deterministic bounded encoding;
- refusal when an occupied address contains another full identity.

Do not preserve the current key shape. `representation_id` and current `unit_id` are revision/coordinate
bearing, so a small early edit can recreate a large suffix of points. Pre-release S11 bytes and public names
are not compatibility contracts.

## Target identity

For one selected physical generation, derive the full identity from immutable retrieval meaning:

```text
key schema version
collection generation
stable source/projection grouping
occurrence incarnation
retrieval profile and role
exact retrieval-input digest
point role
```

Exact source revision, representation, logical occurrence and byte/line range belong to the pinned
`ViewManifestEntry`, not the Qdrant point identity. Consecutive views may bind the same unchanged retrieval
point to different exact ranges while old views retain their old evidence.

`occurrence_incarnation` is assigned by the structure-first preparation/projection owner. Reuse is allowed
only for an unambiguous one-old/one-new occurrence with the same actual retrieval input. Ambiguous equal
repeats receive new incarnations; do not build a general AST-diff or lineage service to guess identity.

## Operations

### `encode_identity_key(key)`

Encode the selected closed key deterministically under explicit size/count limits. Ordinary JSON, map
iteration order, ad-hoc strings and omitted load-bearing fields are not identity input.

### `derive_full_identity(bytes)`

Compute the full content identity with the selected domain-separated hash.

### `derive_qdrant_address(full_identity)`

Project the full identity into the 128-bit address accepted by Qdrant. The address is a locator, not the
complete identity.

### `compare_observed_identity(expected, observed)`

- vacant address: creation permitted;
- same complete identity: idempotent observation;
- same address with another complete identity or key evidence: collision, never overwrite.

Publication stores and reads back enough complete identity evidence to make this comparison. A payload
digest echo alone is not proof.

## Cutover

Change together:

- this key/decoder/comparator;
- projection point assignment and versioned evidence rows;
- Qdrant payload conversion/readback;
- publication diff/cleanup;
- query point-to-evidence lookup.

Build a new physical generation. Delete the old S11 issuer/decoder/goldens and root legacy FNV/registry after
callers move. Do not mint both profiles, rehash existing IDs in place or add a compatibility migration.
Unsupported old application roots remain untouched.

## Focused behavior checks

After code and minimal compiler/Clippy, verify same immutable retrieval input produces the same identity
within a generation; changed input/profile/incarnation produces another identity; equal occurrences remain
distinct; collisions block mutation; coordinate-only movement changes evidence only; and an old pinned view
still opens its old retained excerpt.
