# Projection planning source and implementation

The planner should turn one verified prepared view into ordered Qdrant work. It is pure planning code,
not another source authority, publication journal or compatibility boundary.

## Current API and cost

| Source API | Actual work |
|---|---|
| [`plan_projection`](src/plan.rs#L373) | Checks unit/vector coverage, builds every point, sorts and returns one whole `ProjectionPlan`; no cancellation/deadline input. |
| [`manifest_canonical_bytes`](src/manifest.rs#L122) | Builds and encodes a complete generic CBOR tree. |
| [`diff_manifests`](src/diff.rs#L28) | Builds two maps, clones create/retain/retire entries and rejects a reused point ID whose full entry changed. |
| [`verify_manifest_reconstruction`](src/lib.rs#L81) | Rebuilds/re-encodes complete manifests before comparing them. |
| [`decode_projection_manifest_data`](src/manifest_read.rs#L29) | Decodes the current format and re-encodes it for byte equality. |

Current limits disagree: planning advertises 100,000 points / 64 MiB, while shared collection/codec limits
cap a whole manifest at 4,096 entries / 8 MiB. Point construction happens before the smaller cap rejects.
Use ordered bounded pages and check size/work before growing each page. Raising constants preserves the
same whole-memory failure.

## Split retrieval points from view evidence

Current identity prevents efficient incremental updates:

- source `UnitId` includes revision provenance, absolute coordinates and ordinal;
- S11 point identity includes representation and Unit ID;
- payload includes source revision, representation and Unit ID;
- a manifest entry mixes immutable retrieval state with current evidence binding;
- `diff_manifests` rejects the same point ID when any mixed field changes.

Replace the mixed entry with two concepts.

### Stable retrieval point

A point key contains only fields that define stored retrieval/filter behavior:

```text
collection generation
projection membership / stable source grouping
occurrence incarnation
retrieval profile and role
exact retrieval-input digest
point role
```

The occurrence incarnation distinguishes duplicate equal passages and changes when an occurrence vanishes
and later returns. Absolute byte/line offsets and source revision do not belong in this key. A retrieval
profile change normally creates a new generation; changed input/context creates a new point.

Qdrant payload contains stable fields used by filtering/grouping plus point identity and validity interval.
Remove version-specific citation coordinates from the payload with planner/bridge/query consumers. Do not
pretend the current S9.5/S11 bytes already implement this model.

### Versioned view binding

Each ordered view-manifest entry binds a point to exact evidence for that view:

```text
point_id
source_id / source membership
source revision and representation
logical occurrence and exact range
prepared-artifact/reference information
validity/view metadata needed by query lookup
```

Consecutive views can retain one unchanged retrieval point while writing a new evidence binding. Old pinned
views keep their old binding. No in-place payload update may change historical citation meaning.

Diff retrieval-point state separately from view bindings:

- point absent before, present now → create;
- same point bytes/profile/filter state → retain;
- changed retrieval input/profile/incarnation → create new and retire old;
- evidence range/revision only changed → retain point and update the new view manifest;
- point absent now → close/retire after old views drain.

## Keep uploadable input and one representation

Current `build_point` hashes borrowed vectors and returns only identity, payload bytes and expected digests;
`PointSpec` drops uploadable values. Keep the prepared text/vector input available to the same bounded
Qdrant batch. Native BM25 carries text/options; selected client encoders carry actual vectors. Never
recompute BM25 or treat a digest as vector data.

Build/check each manifest page once and let publication/view storage retain those bytes. Remove whole old/new
manifest embedding in another prepared digest, repeated reconstruction and full-map diffing. A merge walk
over ordered pages is enough. New internal formats can replace current canonical bytes with their writer and
readers together; unsupported old roots remain untouched.

## Integration result

The caller supplies one verified prepared artifact, accepted scope/profile and current view. The planner
returns bounded pages containing:

- complete/narrow Qdrant mutation input;
- new view bindings;
- exact retired IDs or retired generation;
- expected readback fields actually checkable from Qdrant;
- cancellation/progress checkpoints between pages.

Publication owns durable pending/visible state and Qdrant effects. Query owns evidence lookup. The planner
creates neither receipts nor another point/schema owner.
