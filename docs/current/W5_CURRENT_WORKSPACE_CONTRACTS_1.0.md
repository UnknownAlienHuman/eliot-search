# W5 current workspace and Rust structure contracts 1.0

**Status:** implementation projection only; W5 remains blocked.  
**Architecture:** ELIOT Search 8.4, S7, S15-S19, S24, H3-H7, P09-P10.  
**Rule:** package-local `FUNCTIONS.md` owns operation-level semantics; this file owns cross-package
currentness and transient-state invariants.

## 1. Four planes must remain distinct

```text
source truth plane
  exact filesystem/Git bytes or immutable admitted SourceRevision

observation plane
  watcher hints, inventory cursors, gaps and reconciliation receipts

overlay plane
  saved-revision and explicit unsaved-buffer transient products

projection plane
  committed Qdrant epochs/routes built from immutable revisions
```

A plane may cover a lag in another plane but never rewrites its truth:

- a watcher event does not create a source revision;
- a Qdrant epoch does not prove filesystem currentness;
- an unsaved buffer does not alter disk/source truth;
- a saved overlay does not make the base projection current;
- syntax enrichment is rebuildable metadata and never source truth.

## 2. Watcher events are hints

`search-source-reconcile` may consume bounded watcher notifications to prioritize work. A notification
contains only enough locator/root/event/cursor metadata to schedule verification. It does not prove
existence, identity, deletion, rename, content, access, currentness or revision.

Every event is re-resolved under the current admitted-root policy and source identity rules. Coalesced,
duplicate, out-of-order and missing notifications are expected operating conditions.

The watcher adapter is replaceable and private. Native event/vendor types do not cross the reconciler's
public API.

## 3. Observation gap state

A gap is created before acknowledging any condition that may have lost observation continuity:

- watcher overflow or dropped events;
- cursor discontinuity/reset/wrap/unsupported change;
- root unavailable or access denied during required observation;
- daemon/process owner change without a validated cursor resume;
- inventory cancellation/failure after a gap was declared;
- registry/root revision changing during a reconciliation sweep;
- bounded journal/checkpoint corruption or ambiguity.

```yaml
ObservationGap:
  gap_id: OpaqueId
  owner_epoch: OwnerEpoch
  root_binding_id: RootBindingId
  started_after_cursor: ObservationCursorRevision | null
  detected_at: UtcTimestamp
  reason: ObservationGapReason
  affected_scope_digest: Blake3Digest32
  reconciliation_required: bool
  state: open | reconciling | resolved | superseded
  gap_digest: Blake3Digest32
```

An open/reconciling/ambiguous gap immediately prevents `current_confirmed` for its affected workspace/
source scope. It may coexist with source-backed older evidence carrying explicit age/gap state. It cannot
be suppressed by a long polling interval or successful point query.

Gap resolution requires a complete authoritative reconciliation receipt over the affected admitted
scope and a guarded control commit. Receiving another watcher event never resolves a gap.

## 4. Authoritative inventory reconciliation

A reconciliation plan freezes:

```yaml
ReconciliationPlan:
  plan_id: OpaqueId
  owner_epoch: OwnerEpoch
  root_binding_id: RootBindingId
  root_registration_revision: NonZeroRevision
  source_registry_revision: NonZeroRevision
  admission_policy_revision: PolicyRevision
  start_cursor: ObservationCursorRevision | null
  affected_gap_id: OpaqueId | null
  inventory_strategy: full | bounded_subtree_with_complete_parent_proof
  slice_limits: ReconciliationLimits
  plan_digest: Blake3Digest32
```

Baseline gap recovery uses a complete admitted-root inventory. Subtree optimization is legal only when
it proves unchanged parent/root continuity and exact coverage; otherwise it falls back to full inventory.

Inventory proceeds in deterministic bounded slices. For every observed locator it records final-handle
root containment, physical/logical identity observations, metadata needed by admission/identity and an
opaque stable-read/revision candidate reference. The reconciler does not itself define source identity,
read bytes unsafely or apply admission rules.

Directory traversal, entry count, byte/metadata accumulation, time, retry and concurrency are finite.
Ordering is canonical and independent of OS enumeration order.

## 5. Inventory outcomes and atomic reconciliation

Only after complete traversal does the reconciler classify:

```text
observed unchanged locator/identity
new locator candidate
possible rename/move
hardlink/alias candidate
changed revision candidate
unseen prior locator/source
root unavailable/denied
unstable or identity-ambiguous entry
```

`search-source-identity` remains the sole owner of physical/logical identity, path history and cutover
state. `search-source-registry` remains the sole owner of roots/memberships/source views and namespace
cutover. The reconciler emits vendor-neutral commands/receipts to those owners; it does not mutate their
stores directly.

An unseen prior source is not removed merely because one partial slice omitted it. Removal/tombstone or
membership transition requires complete inventory coverage under the frozen root/registry/admission
revisions and guarded application.

The final guarded control transaction verifies:

- owner epoch;
- root registration/admission policy revisions;
- registry/source identity generations;
- start/end observation cursor assumptions;
- complete inventory manifest digest;
- no unresolved conflicting reconciliation.

On guard failure, no `current_confirmed` receipt is published. The plan is replanned or leaves an open
gap.

## 6. Currentness model

Currentness is multidimensional and scope-bound:

```yaml
WorkspaceCurrentness:
  workspace_id: WorkspaceId
  workspace_view_revision: WorkspaceViewRevisionId
  observation_state: current_confirmed | gap_detected | unknown
  reconciled_root_receipts: bounded_list<ReconciliationReceiptRef>
  source_view_digest: Blake3Digest32
  saved_overlay_revision: OverlayRevision | null
  unsaved_buffer_snapshots: bounded_map<SourceId, BufferSnapshotId>
  projection_route_revision: CollectionRouteRevision | null
  projection_visible_epoch: Epoch | null
  projection_lag_state: current_to_saved_revision | covered_by_saved_overlay | stale | unavailable
  evaluated_at: UtcTimestamp
  currentness_digest: Blake3Digest32
```

Claims are precise:

- **current to filesystem/Git:** every affected admitted root has continuity and complete reconciliation
  at the stated cursor/source-view revision;
- **current to saved revision:** exact saved `SourceRevisionId` is source truth and either published in
  the visible projection or covered by an active saved overlay;
- **current to buffer snapshot:** explicit binding-scoped `BufferSnapshotId`, not disk/workspace truth;
- **projection current:** visible Qdrant route/epoch matches the exact published revision/profile set;
- **unknown/gap:** no current claim, even if older source-backed results remain usable.

Unsaved edits must not make the overall disk workspace “current.” The response distinguishes saved
workspace currentness from buffer-snapshot currentness.

## 7. Saved-revision overlays

A saved overlay covers an immutable admitted `SourceRevisionId` that is newer than, absent from or
otherwise not yet represented by the currently visible projection.

```yaml
SavedOverlayEntry:
  overlay_entry_id: OpaqueId
  owner_epoch: OwnerEpoch
  workspace_id: WorkspaceId
  source_id: SourceId
  source_revision_id: SourceRevisionId
  source_owner_generation: SourceOwnerGeneration
  representation_id: RepresentationId
  unit_manifest_ref: OpaqueRef
  lexical_profile_id: ProfileId
  local_projection_digest: Blake3Digest32
  shadowed_base_revision_ids: bounded_set<SourceRevisionId>
  created_at: UtcTimestamp
  expires_at: UtcTimestamp | null
  state: active | superseded | published | revoked | purged
```

Saved overlay derives only from an exact immutable revision/readback receipt. It may retain bounded
rebuildable prepared units/vectors according to accepted storage policy, but it is not a second durable
search database and cannot become canonical evidence independently of source revision readback.

The overlay is retired only after the base publication/route visibly represents the same source
revision and accepted profile, or after explicit invalidation. A newer publication for another revision
cannot silently mark it covered.

## 8. Explicit unsaved buffer snapshots

Unsaved bytes enter Search only through an authenticated explicit snapshot command:

```yaml
UnsavedBufferSnapshot:
  buffer_snapshot_id: BufferSnapshotId
  owner_epoch: OwnerEpoch
  binding_id: BindingId
  workspace_id: WorkspaceId
  source_id: SourceId | null
  base_source_revision_id: SourceRevisionId | null
  client_buffer_revision: NonZeroRevision
  content_digest: Blake3Digest32
  byte_length: u64
  encoding: explicit_tag
  admission_receipt: OpaqueRef
  created_at: UtcTimestamp
  expires_at: UtcTimestamp
  state: active | superseded | saved | closed | expired | revoked | purged | owner_changed
```

Required properties:

- binding/workspace/source scope is explicit and currently authorized;
- content bytes and length are bounded before allocation;
- encoding is explicit; no executable content, hook, filter, build or network action runs;
- admission policy is evaluated for the buffer class without pretending disk metadata exists;
- equal command identity/input is idempotent; conflicting revision/digest is rejected;
- newer buffer revision atomically supersedes the older active entry;
- unsaved bytes remain in bounded process memory only.

Forbidden persistence paths:

```text
redb rows
CAS/source revision store
Qdrant points/payload/vectors
ordinary logs/metrics/crash diagnostics
backups or durable continuation/handle records
```

A process crash/restart invalidates all unsaved buffer snapshots by owner epoch. They are not recovered
from disk. The client must resubmit and receive a new admission/currentness identity.

## 9. Shadow fence before retrieval and IDF

For every active saved/unsaved overlay, `search-overlay` computes a canonical `OverlayShadowFence`:

```yaml
OverlayShadowFence:
  overlay_revision: OverlayRevision
  owner_epoch: OwnerEpoch
  binding_id: BindingId | null
  workspace_id: WorkspaceId
  shadowed_source_ids: bounded_set<SourceId>
  shadowed_revision_ids: bounded_set<SourceRevisionId>
  shadowed_projection_membership_ids: bounded_set<ProjectionMembershipId>
  shadowed_unit_or_range_digest: Blake3Digest32
  fence_digest: Blake3Digest32
```

The fence is part of the planning snapshot/base eligibility and applies before base retrieval, IDF,
facets, counts and traces. It prevents duplicate/stale base points from affecting scoring while the
overlay leg represents the newer saved/buffer view.

Post-candidate duplicate removal cannot repair base IDF/ordering contamination. A missing/stale/ambiguous
shadow fence makes the affected combined leg ineligible or forces replan; it does not silently run both.

Unsaved shadow fences are binding-scoped. One client's buffer cannot alter another binding's source view
or scoring population.

## 10. Overlay preparation

Overlay preparation reuses accepted pure materialization/unitization/lexical behavior through public
contracts:

```text
exact saved SourceRevision or admitted UnsavedBufferSnapshot
  → bounded representation/materialization
  → deterministic units/native anchors
  → accepted lexical profile encoding
  → local bounded overlay index/list
```

No Qdrant mutation occurs. The local structure is bounded by source count, bytes, units, features,
expiry and binding quotas. It may use deterministic in-memory maps/heaps but is not a general persistent
index or alternate search database.

Every prepared entry binds exact source/buffer identity, representation/unit/profile digests and shadow
fence. Preparation cancellation yields no active partial entry. Partial parsing/unitization is explicit
with assurance/gaps and cannot hide the base revision without a valid replacement policy.

## 11. Overlay query leg

The overlay leg consumes the immutable plan, effective grant, live security fence, overlay snapshot and
finite leg budget. It returns bounded nominations with overlay/buffer/revision/unit/profile identities.

Saved and unsaved populations are distinct from Qdrant/base populations. Raw scores are not directly
compared across them. Cross-leg fusion uses the accepted rank-based profile.

Before every nomination and before result validation:

- binding/grant/live deny/purge/membership are rechecked;
- overlay entry and shadow fence remain active/current;
- unsaved buffer revision/owner epoch/TTL still match;
- saved source revision remains available;
- output stays inside authorized workspace/source/disclosure scope.

Nominations still undergo the normal source-backed validator. For unsaved entries, the exact active
buffer snapshot is the source truth plane for that binding only; it remains ephemeral and cannot produce
a durable handle.

## 12. Overlay lifecycle and invalidation

Monotonic transitions include:

```text
unsaved: active → superseded | saved | closed | expired | revoked | purged | owner_changed
saved:   active → superseded | published | expired | revoked | purged | owner_changed
```

Triggers:

- newer buffer/saved revision;
- editor save/close/disconnect;
- binding/grant/access revocation;
- purge or membership/source removal;
- owner epoch change;
- TTL/quota reduction;
- matching base publication becoming visible;
- lexical/profile/source-view incompatibility.

Invalidation publishes a new overlay/shadow revision before acknowledgement. Matching requests,
continuations and ephemeral handles are revalidated/invalidated. Bytes, vectors and units are removed
from memory on terminal unsaved transitions, subject only to safe buffer zeroization/release semantics;
no physical secure-erase claim is made.

## 13. Rust syntax enrichment profile

`search-code-enricher` owns a qualified immutable syntax profile:

```yaml
CodeEnrichmentProfile:
  profile_id: ProfileId
  language: rust
  parser_provider_ref: QualifiedDependencyRef
  parser_grammar_version: BoundedText
  parser_artifact_digest: Sha256Digest32 | Blake3Digest32
  query_or_walker_revision: NonZeroRevision
  normalization_revision: NonZeroRevision
  cfg_observation_policy: ProfileId
  assurance_mapping_revision: NonZeroRevision
  input_limits_ref: OpaqueRef
  golden_fixture_digest: Blake3Digest32
  profile_digest: Blake3Digest32
```

No parser/grammar version is accepted merely because it compiles or is common. Qualification freezes
exact dependency source/version/checksum/license, malformed/cfg/macro fixtures and span behavior.

Profile changes require re-enrichment and replacement/rebuild of affected projections. They cannot
reinterpret existing facts in place.

## 14. No-execute Rust parsing

The enricher receives exact admitted source-revision representation bytes plus coordinate maps. It
never runs:

```text
Cargo/build scripts
rustc
procedural/declarative macro expansion
code generation
hooks/formatters/linters
network or package resolution
credential prompts
```

It emits syntax/descriptive facts only:

- module/item hierarchy and native spans;
- function/method/type/trait/impl/field/constant/static/macro definitions;
- explicit use/import paths and syntactic references;
- impl target/trait syntax where present;
- test/configuration/definition evidence roles;
- `cfg`/`cfg_attr` expressions as bounded descriptive syntax;
- comments/doc associations when the profile explicitly qualifies them;
- parse errors, missing nodes and unsupported/macro-generated uncertainty.

It does not claim name resolution, type resolution, trait selection, call graph certainty, macro-expanded
semantics, compiled cfg inclusion, dead-code status or behavior equivalence.

## 15. Assurance and malformed input

Every fact carries an assurance class and exact source anchor:

```yaml
CodeFact:
  fact_id: OpaqueId
  source_revision_id: SourceRevisionId
  representation_id: RepresentationId
  unit_id: UnitId | null
  entity_kind: EntityKind
  normalized_symbol_key: BoundedSymbolKey | null
  fact_kind: ProfileId
  native_anchor: NativeAnchor
  cfg_expression: BoundedExpression | null
  evidence_role: EvidenceRole
  assurance: exact_bytes | mapped_text | descriptive_only
  profile_id: ProfileId
  fact_digest: Blake3Digest32
```

Parser offsets are mapped back through coordinate/loss maps to native source. A fact without a valid
bounded anchor cannot be projected as exact syntax evidence.

Malformed or incomplete source may yield bounded partial facts only when:

- parser explicitly identifies recovered/error regions;
- each retained fact lies outside or records intersection with uncertainty;
- gaps/unknowns are explicit;
- result never claims complete structure or semantic absence.

A parse failure does not execute another tool automatically and does not silently fall back to regex
with equal assurance.

## 16. Structure integration

Enrichment is rebuildable metadata attached to exact source revision/representation/unit/profile. It may
feed exact-symbol branches, structural candidate nomination, subject resolution and comparison after the
relevant later package gates.

W5 itself does not create complete negative proofs. Absence of a syntax fact can mean unsupported syntax,
macro generation, cfg uncertainty, malformed input or profile limitation. Exact absence remains the
exact/source scan owner's decision.

Access and currentness still apply before structure retrieval/scoring. Syntax metadata cannot expose an
inaccessible symbol/path/name before authorization.

## 17. Configuration interaction

`reconcile` and `overlay` are existing capability-owned sections. Rust enrichment profile/limits remain
qualified/internal W5 settings until explicitly admitted.

Locked invariants:

- watcher is not authority;
- gap blocks current-confirmed;
- overflow requires reconciliation;
- unsaved bytes never persist;
- unsaved state is binding/owner/TTL scoped;
- shadowing applies before base retrieval and IDF;
- overlay is not a second durable database;
- code enrichment executes no build/macro/network action;
- syntax assurance never claims compiler semantics.

Tunable values are finite inventory intervals/slices, overlay source/byte/unit/TTL quotas and parser
input/node/depth/error limits. Restrictive reductions may pause work or invalidate excess transient
state; they never falsify currentness or persist unsaved data.

## 18. Cancellation, crash and recovery

### Reconciliation

Cancellation before complete inventory leaves currentness unresolved and preserves/open a gap. Bounded
checkpoint may resume only if owner/root/registry/admission/cursor guards still match; otherwise it is
superseded and replanned. Unknown guarded apply outcome is resolved from control operation/receipt state.

### Overlay

Preparation/query cancellation yields no active partial entry/result. Unsaved state disappears on crash.
Saved overlay metadata may be rebuilt from immutable revision/receipts; deserialized state is revalidated
before activation. Unknown update/invalidation outcome is resolved by operation identity and overlay
revision.

### Code enrichment

Parsing is pure/read-only. Cancellation yields no successful complete manifest; bounded partial output
must satisfy the explicit partial-assurance contract. Equal revision/profile input gives equal facts and
manifest digest.

## 19. Cross-package failure ownership

| Failure | Sole deciding owner |
|---|---|
| watcher cursor/gap/inventory completeness/currentness receipt | `search-source-reconcile` |
| physical/logical source identity/path history/cutover | `search-source-identity` |
| root/membership/source-view registry transition | `search-source-registry` |
| final-handle stable byte acquisition | `search-safe-reader` |
| saved/unsaved overlay state, shadow fence and local nominations | `search-overlay` |
| exact immutable revision/anchor readback | `search-revision-store` |
| Rust syntax profile/facts/assurance | `search-code-enricher` |
| access and result interpretation | existing W4 owners |
| projection visibility | `search-publication` |

Consumers preserve producer-typed failures and do not create a second state machine/store.

## 20. W5 exit evidence

W5 cannot pass from compilation alone. Required raw evidence includes:

- watcher duplicate/out-of-order/coalesced/overflow/reset cases;
- gap published before overflow acknowledgement and blocks currentness;
- complete bounded inventory and partial-slice non-removal;
- registry/root/cursor guard-race and crash/resume fixtures;
- rename/hardlink/reparse/unavailable-root integration through identity/reader ports;
- saved overlay covers exact newer revision until matching publication;
- explicit unsaved snapshot admission, update/save/close/restart lifecycle;
- proof unsaved bytes never reach redb/CAS/Qdrant/logs/backups;
- binding-scoped shadowing before retrieval and IDF with cross-binding noninterference;
- bounded overlay memory/units/features/query and deterministic fusion inputs;
- revocation/purge/TTL/quota/profile/publication invalidation receipts;
- exact Rust parser dependency/profile qualification;
- malformed/cfg/macro/span/Unicode/CRLF/UTF-16 Rust fixtures;
- no-execute/no-network/no-build proof;
- syntax assurance overclaim rejection;
- truthful disk/saved/buffer/projection currentness report.

Until these exist, W5 remains `BLOCKED` or `UNAVAILABLE`, never `PASS`.
