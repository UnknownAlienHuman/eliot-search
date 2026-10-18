# Projection planning source and implementation

## Current API

| Source API | Actual work |
| --- | --- |
| [`validate_scope_binding`](src/plan.rs#L64) | Checks the supplied scope against the verified unit set and point budget. |
| [`plan_projection(unit_set, scope, planned_units, requirements, budget)`](src/plan.rs#L373) | Checks complete unit/vector coverage, builds every point, sorts them and returns a whole `ProjectionPlan`. It has no cancellation/deadline parameter. |
| [`manifest_canonical_bytes`](src/manifest.rs#L122) | Builds a complete generic CBOR value tree and encodes it under both caller and shared limits. |
| [`diff_manifests`](src/diff.rs#L28) | Validates already sorted inputs, builds two maps and clones create/retain/retire entries. Reused IDs with changed immutable content are rejected. |
| [`verify_manifest_reconstruction`, semantic-digest helpers](src/lib.rs#L81) | Rebuild or re-encode complete manifests before comparing/hashing them. |
| [`decode_projection_manifest_data`](src/manifest_read.rs#L29) | Decodes the current stored format and re-encodes it for byte equality; does not prepare source data. |
| [`validate_projection_manifest_metadata`](src/manifest_read.rs#L74), [`validate_schema`](src/lib.rs#L140) | Check metadata/scope/schema consistency. They neither fetch indexed data nor establish that publication happened. |

## Concrete changes

**Make capacity follow the stored data.** [`ProjectionBudget::BASELINE`](src/model.rs#L32)
sets 100,000 points and 64 MiB. Encoding rejects more than
[`MAX_COLLECTION_ITEMS = 4,096`](../../search-contracts/src/bounds.rs#L9)
or [8 MiB](../../search-contracts/src/bounds.rs#L8). This is one membership manifest,
not the entire corpus. Point construction occurs before the row-cap rejection,
and the encoded-size check follows tree construction. Store/iterate an ordered
manifest in bounded pages; check the current piece's size before growing it.
Keep total completeness while allowing cancellation between pieces. Merely raising
these constants retains the same full-memory path.

**Keep actual vectors available for upload.** [`build_point`](src/plan.rs#L269)
validates/hashes the borrowed vectors, then returns identity, payload bytes and
expected digests. [`PointSpec`](src/model.rs#L143) has no vector values. Connect
that metadata to the same prepared vector product when building a Qdrant batch;
do not regenerate vectors, treat a digest as data or add another preparation proof.

**Remove repeated representations.** [`rebuild_manifest`](src/plan.rs#L407)
clones entries into a temporary manifest and copies the resulting bytes.
Publication admission reconstructs the result again, then
[embeds both manifests in a prepared digest](../search-publication/src/admission.rs#L501).
Build/check the representation once and borrow it. Walk sorted old/new entries
instead of rebuilding two `BTreeMap` indexes and cloning retained entries; an
[existing merge walk](../search-publication/src/machine.rs#L556) provides the pattern.

**Read the active format directly.** Keep a bounded stored-metadata reader for
actual consumers. Replace its format with the writer when simplifying storage;
pre-release byte compatibility and mandatory old-format readers are not a
prerequisite. Useful content identity does not require a digest of every transient
plan, a second canonical tree or repeated re-encoding in unrelated consumers.

These changes join the existing planner, publication and bridge. They introduce
no alternate search backend or additional planning framework.
