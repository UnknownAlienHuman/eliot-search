# Projection planner — exact source/caller audit and replacement

The current package is pure code, but it plans the wrong object: one complete revision-bound vector/digest
manifest assembled in memory. The selected application needs bounded publication pages containing actual
Qdrant inputs plus a separate versioned evidence manifest.

## Measured source and caller reality

At public main `c8e38b6`, production `src/` is **64,388 bytes**:

```text
diff.rs             2,120
error.rs            5,088
lib.rs              6,080
manifest.rs         5,866
manifest_read.rs    9,348
model.rs            9,583
plan.rs            18,288
point.rs             3,599
validate.rs          4,416
```

Repository searches find:

- `plan_projection` used by this package's tests and publication tests/support;
- `diff_manifests` called by publication admission plus tests;
- `decode_projection_manifest_data` used only by this package's tests/docs;
- `PublicationAdmissionOwners` implemented only in publication test modules/support, not by daemon/control/
  preparation production code.

Thus the planner/publication validation stack is not connected to a real preparation producer, durable
publication owner or Qdrant uploader. Passing its fixtures does not establish an application path.

## Current plan loses the data needed for publication

`PlannedUnit` receives complete `PlannedVector` values. `build_point` validates them and constructs canonical
per-vector digests, but returned `PointSpec` retains only:

```text
point identity
canonical payload bytes + payload digest
vector-name → digest map
unit/reference digests
```

Actual vector values are dropped. No text/options exist for native Qdrant BM25. Publication can validate
expected digests but cannot upload the data represented by the plan without another computation or an
out-of-band producer.

The replacement page retains the actual operation input until exact Qdrant apply/readback:

```text
PointMutation {
  stable point identity + collision evidence
  complete or narrow selected payload fields
  native BM25 Document text/options and/or actual selected vector values
  expected observable Qdrant state
}

ViewBinding {
  point_id
  exact retained ObjectRef/revision
  representation/logical occurrence/range
  prepared-artifact reference and verification data
}
```

A digest may identify an input/artifact, but never substitutes for uploadable text/vector values.

## Advertised limits are false

`ProjectionBudget::BASELINE` advertises 100,000 points and 64 MiB. The manifest codec also enforces global
`MAX_COLLECTION_ITEMS = 4,096` and `MAX_CANONICAL_BYTES = 8 MiB`. `plan_projection` builds and sorts every
point before the later whole-manifest encoder rejects the smaller limit.

Do not raise all constants. Replace whole-corpus planning with bounded pages. Validate page point/encoded-byte
limits before reserve/growth and checkpoint cancellation/deadline between pages.

## Current identity and manifest amplify edits

`derive_scoring_document_id` hashes/truncates:

```text
source_revision_id
representation_id
unit_id
projection_profile_set_id
```

The 128-bit result drops the remaining digest and therefore is not demonstrably collision-safe by itself.
More importantly, revision/representation/current Unit ID make it change on harmless source movement.
`ProjectionPointKey` also includes representation and Unit ID.

`ProjectionManifestEntry` then repeats per point:

```text
source/projection membership
collection generation
installation incarnation
access/scoring partitions
profile
unit/reference/payload/vector digests
```

Many are one scope/header fact or belong to the stable point, while the entry still lacks the exact retained
`ObjectRef` and evidence range needed by query.

Replace this mixed record:

```text
stable retrieval point:
  collection generation
  stable source/group membership
  occurrence incarnation
  retrieval profile/role/input identity
  point role and selected filter/group fields

versioned view binding:
  view sequence
  point ID
  exact retained source ObjectRef/revision
  representation/logical occurrence/range
```

A top-of-file insertion may retain the point and write a new binding. Changed retrieval input/profile or a
new occurrence incarnation creates a new point. Old pinned views keep old bindings.

## Whole-memory and repeated work

Current planning performs multiple full copies:

- builds all point specs;
- derives all manifest entries;
- clones the complete entry vector;
- builds a generic `CanonicalValue` tree;
- serializes and stores another full byte copy;
- decode copies the encoded bytes and re-encodes the complete manifest for equality;
- diff converts already sorted vectors to two `BTreeMap`s and clones create/retain/retire entries;
- publication admission walks complete units/points/manifests and computes artificial allocation work again.

Use ordered iterators and immutable pages instead:

```text
prepared-unit stream sorted by stable point identity
+ old point/view page iterator
→ bounded new point-operation page
→ bounded new view-binding page
→ ordered create/retain/retire result
→ immutable artifact write
```

A two-way merge over sorted page iterators is linear and bounded. It does not need two maps. Point-state diff
and evidence-binding diff are separate: changed evidence does not make stable point reuse a collision.

## Target page model

One publication operation artifact contains a small header and ordered pages:

```text
ProjectionOperationHeader {
  target route/generation/profile/view sequence
  stable selected source/group population
  page counts / exact current-format codec version
}

ProjectionPage {
  PointMutation[]
  ViewBinding[]
  ClosePointId[]
  expected Qdrant readback fields
}
```

Store scope constants in the header, not every entry. Keep the current-format page codec simple and versioned
with its reader/writer together. A sorted explicit binary or bounded Serde record is sufficient; do not build
another global canonical-value framework. The immutable object hash identifies exact page bytes.

Publication persists the artifact reference/cursor, applies one page, reads actual Qdrant state and advances.
Query loads the published view-binding page/index needed for selected points. Planner owns no redb/Qdrant
mutation, current view, receipt or recovery state.

## Direct preparation input

Do not require a duplicate `PlannedUnit[]` parallel to `VerifiedUnitSet`. Preparation should expose one
current iterator/record containing:

```text
occurrence incarnation and stable structural anchor
exact evidence binding
bounded retrieval input text and/or selected actual vectors
retrieval role/profile
stable filter/group values
```

The planner validates each unit once while building the page. Native BM25 uses retained text/options; optional
client encoders provide actual finite vectors. It does not precompute BM25 token IDs/weights/digests locally.

## Publication admission simplification

The current `PublicationAdmissionOwners` trait has only test implementations. Its production API asks a
caller to provide projection/control snapshots, then performs an artificial allowance calculation:

```text
(payload + manifest bytes) * 16
+ units * 8192
+ 32768
```

`AdmissionWork::Allocate/Process` does not reserve the allocations it predicts. Remove this proof seam.
The real publication owner receives one verified immutable operation artifact plus expected current view,
checks its actual header/page bounds and persists `PendingPublication`. It observes current redb/Qdrant state
directly during apply/recovery.

## Remove after cutover

- whole `ProjectionPlan` / complete `ProjectionManifest` ownership;
- caller-supplied membership/lineage/proof scope records not read from current owners;
- precomputed vector requirements/digests as substitutes for actual inputs;
- revision-bound scoring-document identity and current S11/S9.5 point construction;
- full canonical payload/manifest preimages and reconstruction proof;
- full-map `ManifestDiff` and same-point/evidence-change collision rule;
- `PublicationAdmissionOwners`, `AdmissionWork` and fixture-only owner implementations;
- planner/publication fixtures whose only subject is these exact bytes/proofs.

Keep or rewrite only substantial pure algorithms: stable identity input construction, bounded ordered page
building, actual vector/text shape checks and merge-walk diff. If this is small after cutover, merge it into
preparation/publication instead of preserving a crate boundary.

## Verification

Focused cases:

- insertion near file start retains unaffected point identities and changes only new view bindings;
- duplicate equal occurrences stay distinct; disappear/reappear gets a new incarnation;
- changed retrieval context/profile creates a new point;
- native BM25 text/options and optional vectors reach the actual Qdrant request unchanged;
- page ceilings are enforced before growth and complete membership survives page boundaries;
- old/new page merge produces exact create/retain/close without whole-corpus allocation;
- old pinned and new views reopen their own exact retained ranges;
- publication resumes from one immutable page/cursor after an unknown external outcome.

Measure retained/created/closed points, bytes encoded, peak page memory and Qdrant writes—not only cache hits
or manifest digest equality.
