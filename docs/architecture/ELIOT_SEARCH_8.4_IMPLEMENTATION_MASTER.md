# ELIOT Search 8.4 — Standalone Architecture, Implementation Handoff, and Audit
## Single authoritative implementation master for the Search repository

**Status:** implementation candidate; runtime, performance, security execution, migration, and product acceptance remain unproven.  
**Scope:** ELIOT Search only: standalone local-data preparation and retrieval, plus optional client adapters. Online research, client memory, task control, and canonical knowledge remain outside this repository.  
**Repository authority:** this file is normative for the ELIOT Search repository. No external ELIOT document is required to build or test the standalone core.  
**ELIOT compatibility baseline:** English final Architecture/Implementation pair dated 2026-08-28 (`M1 c6932eaf26935e752eefb4de591afc91ea1a7180be5a8ff0005554b8029bac1a`, `M2 7805bf238fe91819aba50d7e13aa86a8b977561195dbb98aa979f986e2fab063`). The optional adapter profile in S32.3 incorporates the required boundary directly.  
**Original donor SHA-256:** `684315a0b4ce9da007c83180ae60bee62715d488e6f588b64940cb2f5a70b4e4`.  
**Aligned English donor SHA-256:** `9ee370c46834c5f68771bae2848781e3695cf0acc912814b9862cfdb94b5fcdb`.  
**Supersedes:** every separate Search architecture, handoff, audit, patch, validation, and release-manifest file produced before this master.  
**Architecture section SHA-256:** `ae4c18ccff256ce4d5fdf91dfd9041236ff6f332b611bae3bd748c2da8ac6a1c`  
**Codex entry point:** Part II, P00 only.

This file is intentionally self-contained. Codex and reviewers MUST NOT require a second Search Markdown document. Upstream product documentation and optional client-system architecture remain evidence and compatibility inputs, not additional Search contract files.

## Contents

1. Part I — ELIOT Search Architecture 8.4
2. Part II — Codex Handoff 2.7
3. Part III — Consolidated audit register D-1–D-7 and A-01–A-61
4. Part IV — Mechanical validation and release notes

---

# Part I — Architecture

<!-- BEGIN EMBEDDED ARCHITECTURE -->
# ELIOT Search Architecture 8.4
## Qdrant-Only Retrieval, Epoch-Safe, Micromodular, Implementation-Ready

**Status:** implementation candidate for Codex P00; runtime and product acceptance are not yet proven.  
**Supersedes:** ELIOT Search Architecture 8.3 and every earlier Search architecture, storage and publication contract.  
**Does not supersede:** any client-system architecture, online research product, or external evidence/admission policy.  
**Normative language:** MUST, MUST NOT, SHOULD, MAY have their RFC-style meanings.

---

## S0. Executive decision

ELIOT Search is a standalone local-data preparation and retrieval product. It may serve its CLI/IDE clients, an online research system, or ELIOT through optional adapters. It is not a memory system, research orchestrator, task controller, canonical knowledge store, or authority service. The core repository builds, tests, and runs without importing ELIOT or Eliot Research internals.

The baseline topology is deliberately narrow:

```text
user sources / Git objects / authenticated IDE buffers
                         │
                         ▼
                 eliot-searchd.exe
  identity · inventory · revisions · preparation · policy
  publication · exact scan · query recipes · readback · results
        │                  │                    │
        │                  │                    └─ in-memory overlays/pins
        │                  └─ filesystem CAS: immutable revision/materialization manifests
        └─ redb: bounded control journal only
                         │
                         ▼
                    qdrant.exe
       the only search/index database of ELIOT Search
```

The following are prohibited in the baseline and in later profiles unless a new architecture decision replaces this one:

```text
SQLite / FTS5
Tantivy / Zoekt
another BM25 store
another vector store
a client-system canonical-store search catalog
custom ANN or custom inverted-index storage
raw Qdrant access from agents, CLI, workers, or any client adapter
```

Qdrant owns rebuildable retrieval projections. It does not own source truth, source history, access authority, publication truth, exact proof denominators, client canonical state, or interpretation.

A small redb journal is permitted because cross-process lifecycle and crash recovery require durable control state. It MUST NOT expose a retrieval API or hold the searchable corpus.

---

## S1. Product boundary and client profiles

### S1.1 ELIOT Search core

ELIOT Search is a separate repository and deployable product. For every source namespace it admits, it is the sole authoritative mutable owner of:

- source identity, path history, and source-revision occurrences;
- safe no-execute reads and coherent revision readback;
- materialization, unitization, code/document enrichment, and coordinate/loss maps;
- exact, lexical, structural, and optional semantic retrieval projections;
- currentness, publication, overlays, local handles, purge, rebuild, and provider capability state.

Search returns prepared candidates, navigation, coverage, freshness, and uncertainty. It does not decide what a client should believe, admit, publish, or treat as complete.

### S1.2 Standalone clients

The CLI, IDE integration, and future local applications use the same daemon, grant compiler, recipes, and result contracts as every other client. They do not open redb or Qdrant directly and do not gain authority by running on the same machine.

### S1.3 Optional ELIOT adapter

The optional ELIOT adapter maps ELIOT-owned scope, authority, and verification contracts to the generic Search provider boundary. It is a leaf adapter, disabled by default, and MUST NOT:

- import ELIOT storage or canonical-writer code into the Search core;
- receive canonical database credentials, task authority, admission authority, or finish authority;
- mint ELIOT memory dispositions or mutate ELIOT evidence;
- create a second mutable source catalogue for a namespace owned by Search.

The complete compatibility profile is embedded in S32.3; the Search core does not require the full ELIOT repository.

### S1.4 Research systems

An online research system may use Search as an explicit preparation/export provider. Search does not implement an online research service, call one as an implicit fallback, share its database, or synthesize final research conclusions. A transfer preserves source-owner identity and revision lineage; ownership changes only through an explicit cutover receipt.

## S2. Design targets and non-goals

### S2.1 Required balance

| Target | Architectural response |
|---|---|
| Cheap and fast | One retrieval database, no generative model in the baseline, direct/exact first, incremental preparation, bounded cards |
| Easy to develop | Stable recipes, pure domain state machines, thin adapters, one serialized publication coordinator |
| Functional | Exact, lexical, structural, cross-repository comparison, overlays, proof plans, provenance and coverage |
| Modern | Qdrant Query API, filtered IDF, named sparse/dense vectors, optional rerank and multivectors |
| Smart | Subject resolution, analogue ladder, evidence-role alignment, lineage collapse and explicit ambiguity |
| Universal | Source → Revision → Materialization → Representation → Unit → Anchor with modality-specific adapters |

### S2.2 Explicit non-goals

Search 8.4 does not promise:

- arbitrary semantic truth or autonomous architectural judgment;
- perfect compiler semantics from Tree-sitter;
- complete OCR or document fidelity;
- remote web research;
- a durable substitute for client-owned canonical evidence or research records;
- multi-node Qdrant clustering;
- multi-user service tenancy;
- support for every file format in the first product slice.

---

## S3. Non-negotiable invariants

```yaml
SearchInvariants:
  INV-01: Qdrant is the only search/index database.
  INV-02: redb contains control state, never a searchable corpus.
  INV-03: original source or an immutable admitted revision is source truth.
  INV-04: retrieval proposes candidates; the consuming client owns interpretation and admission.
  INV-05: one Qdrant point belongs to exactly one ProjectionMembership.
  INV-06: no point payload contains an array of corpus memberships or names.
  INV-07: access/currentness filters apply before candidate generation and IDF scoring.
  INV-08: Qdrant top-K never narrows an exact negative-proof denominator.
  INV-09: restrictive access and purge fences override query snapshots immediately.
  INV-10: a short query performs no durable control-store write.
  INV-11: an uncommitted epoch is never observable as current.
  INV-12: an epoch number is never reused within a collection generation.
  INV-13: publication has at most one active Qdrant commit transaction globally.
  INV-14: Qdrant writes on the publication path are acknowledged and read back before commit.
  INV-15: no source range is called exact without a declared coordinate basis and revision digest.
  INV-16: an unsaved editor buffer is never inferred from a filesystem watcher.
  INV-17: index loss cannot destroy client-owned load-bearing evidence.
  INV-18: physical CAS reuse is allowed only when every residency/security/lifecycle domain is equivalent; retrieval membership is never shared by default.
  INV-19: server-side text inference is capability-probed, never assumed.
  INV-20: stale or inaccessible candidates are removed before result projection.
  INV-21: one safe IDF leg includes at most one equivalent membership per ScoringDocumentId.
  INV-22: a restrictive policy change invalidates every rank leg it influenced.
  INV-23: publication guards are revalidated atomically at the VisibleEpoch commit point.
  INV-24: “current workspace” is never claimed across an unresolved observation gap.
  INV-25: incompatible collection changes use a new generation and redb route cutover.
  INV-26: one admitted source namespace has exactly one authoritative mutable source-identity/revision owner.
  INV-27: every durable CAS object is keyed by scope, access, confidentiality, encryption-key, retention, erasure, and content-digest domains.
  INV-28: unsaved bytes remain ephemeral and cannot enter durable stores, backups, telemetry payloads, provider caches, evaluation corpora, or learning/training inputs without explicit save/admission.
  INV-29: a client adapter cannot create reverse authority, canonical writes, or an implicit online-research fallback.
  INV-30: source-owner transfer requires an explicit fenced cutover; immutable export is not ownership transfer.
```

A code change that violates an invariant requires a new architecture revision, not a local workaround.

---

## S4. Abstraction layers

```text
L0  standalone and optional client-adapter contracts
L1  typed recipe compilation and orchestration
L2  subject resolution, retrieval, comparison and result projection
L3  access/currentness compilation, overlays and exact verification
L4  publication, projection manifests and Qdrant adapter
L5  source registry, revision acquisition, materialization and enrichment
L6  runtime supervision, redb journal, filesystem CAS and qdrant.exe
```

Dependencies point downward. Adapters never call upward into client-system semantics or import client authority into the core.

---

## S5. Micromodular capability map

A capability cell is a causal responsibility with a contract, owner, failure state and test seam. A cell does not automatically imply a process or crate.

| Cell | Responsibility | Owns | Must not own |
|---|---|---|---|
| C00 Contracts | Versioned external/domain types | schemas, IDs, reason codes | runtime state |
| C01 Runtime Owner | one data-root owner epoch | process lease, lifecycle | retrieval semantics |
| C02 Control Journal | durable technical state | intents, routes, cursors, tombstones | corpus text, ranking |
| C03 Source Registry | roots, locators, memberships | source bindings | access decisions |
| C04 Source Identity | physical/logical identity | identities, path history | corpus policy |
| C05 Change Reconciler | watcher hints + inventory reconciliation | observed changes | source truth |
| C06 Safe Reader | stable no-execute reads | revision acquisition | parsing policy |
| C07 Revision Store | immutable retained bytes/manifests | bounded CAS | search queries |
| C08 Materializer | format to canonical representation | loss map | authority |
| C09 Unitizer | deterministic units | unit occurrences | ranking |
| C10 Code Enricher | definitions/references/config predicates | structural facts | compiler certainty |
| C11 Lexical Encoder | text to named sparse vector | analyzer profile | inverted index |
| C12 Model Provider | optional dense/multivector/rerank | model runtime | canonical decisions |
| C13 Projection Planner | point set and manifest | projection plan | Qdrant transport |
| C14 Point Identity | collision-safe point IDs | canonical key/digest | source identity |
| C15 Qdrant Bridge | Qdrant transport and capability checks | vendor calls | project semantics |
| C16 Publication Coordinator | linearizable current projection | epoch state machine | query interpretation |
| C17 Pin/Reclaimer | in-flight snapshot retention | in-memory epoch/route pins | durable query history |
| C18 Access Compiler | grant to safe retrieval legs | filters and deny fences | client authority |
| C19 Overlay | saved/unsaved current deltas | bounded transient candidates | persistent second index |
| C20 Exact Plane | denominator and executable scans | exact reports | semantic absence claims |
| C21 Subject Resolver | resolve entity under context | ambiguity set | normative choice |
| C22 Query Planner | recipe to bounded legs | execution plan | raw natural-language authority |
| C23 Retrieval Executor | direct/Qdrant/provider legs | candidate streams | final belief |
| C24 Candidate Validator | source readback and stale rejection | validated candidates | admission |
| C25 Comparator | descriptive evidence alignment | behavior matrix | “correct implementation” claim |
| C26 Result Projector | compact cards and handles | response budget | hidden source dumps |
| C27 Continuation Owner | bounded continuation lifecycle | opaque continuation records | indefinite epoch pins |
| C28 Retention/Purge | retirement, tombstones, rebuild | lifecycle receipts | secure-erase guarantees |
| C29 Telemetry/Eval | content-minimized metrics and fixtures | quality evidence | hidden training |
| C30 Client Adapter Edge | optional protocol translation | binding/session mapping | Search internals or client authority |

---

## S6. Storage topology

### S6.1 Qdrant: the only search/index database

Qdrant stores rebuildable retrieval points, payload indexes and named sparse/dense/multivectors. Every agent-facing indexed retrieval goes through `QdrantBridge` and the Search query service.

Qdrant does not store authoritative membership policy, source revision history, query leases, client records, or proof denominators.

### S6.2 redb: bounded control journal

Allowed redb records:

```yaml
ControlJournalScope:
  - installation and data-root owner epoch
  - source roots, path bindings and observed revision heads
  - SourceMembership and policy-binding identifiers
  - materialization and projection-manifest references
  - publication intents, receipts and committed VisibleEpoch
  - collection generation and route
  - watcher/reconciliation cursors
  - shadow, deny, purge and abandoned-publication fences
  - bounded durable job checkpoints
  - content-hash metadata
```

Forbidden redb records:

```text
source bodies or extracted corpus text
postings or BM25 statistics
sparse/dense/multivectors
ranked candidate sets used as a general query store
semantic entity retrieval tables
agent query history as an index
```

A lost or incompatible journal causes a new collection generation and rebuild. Search MUST NOT infer authoritative currentness by reverse-engineering an orphaned Qdrant collection.

### S6.3 Filesystem CAS

The CAS is an immutable payload substrate, not a query database. It may contain:

- raw revision bytes required for coherent readback;
- materialized text/structure;
- native-coordinate and loss maps;
- unit/projection manifests;
- source snapshots explicitly retained by a durable handle or export contract.

A global `cas/<content-digest>` namespace is prohibited. Every durable object uses the complete residency identity:

```text
SearchObjectResidencyKey =
  scope_domain_id +
  access_domain_id +
  confidentiality_domain_id +
  encryption_key_domain_id +
  retention_domain_id +
  erasure_domain_id +
  versioned_content_digest
```

The physical path is derived from a digest of the complete residency key:

```text
cas/<residency-key-digest>/<object-kind>/<prefix>/<content-digest>
```

The `versioned_content_digest` serialization includes the digest algorithm identifier; Search 8.4 uses BLAKE3-256 for native Search objects. Equal bytes may share one physical object only when every domain above is equivalent. Byte equality never permits cross-domain co-residency, ciphertext/key reuse, or coupled retention/erasure. Moving bytes to a different domain is an explicit copy or re-encryption transition with a receipt and a disposition for the old copy; metadata relabeling is insufficient.

Objects have no query language. Rebuildable cache objects may be removed under mark-and-sweep; retained source revisions remain reachable while a visible epoch, durable handle, export, legal hold, or client import contract requires them. A client may import an immutable snapshot under its own governance, but Search does not silently transfer source ownership or delete the client copy.

### S6.4 Source truth

Source truth is one of:

1. exact local bytes with a verified SourceRevision;
2. an immutable Git object;
3. an authenticated immutable IDE BufferSnapshot;
4. an admitted imported snapshot with provenance.

Qdrant text or vectors are never source truth.

---

## S7. Identity model

### S7.1 Installation and collection identity

```yaml
SearchInstallation:
  installation_id: uuid
  installation_incarnation_id: uuid
  data_root_id: uuid
  owner_epoch: nonzero_u64
  active_mode: standalone | managed_client

CollectionRoute:
  collection_generation_id: uuid
  physical_collection_name: opaque_string
  schema_identity_digest: blake3_256
  qualified_qdrant_build: artifact_digest
  committed_visible_epoch: Epoch
```

A restored journal, copied data root or replaced Qdrant directory receives a new `installation_incarnation_id` unless an exact paired recovery manifest proves continuity.

### S7.2 Repository and workspace identity

```yaml
RepositoryLineage:
  lineage_id: uuid
  vcs_kind: git | none
  canonical_remote_fingerprints: [digest]
  fork_relations: [opaque_lineage_id]

WorkspaceInstance:
  workspace_id: uuid
  lineage_id: uuid
  root_binding_id: uuid
  worktree_or_checkout_identity: opaque
```

Nested repositories and submodules remain distinct workspaces.

### S7.2.1 Source namespace ownership

```yaml
SourceNamespaceOwnership:
  source_namespace_id: uuid
  owner_system_id: opaque_stable_provider_identity
  owner_installation_incarnation_id: uuid
  owner_epoch: nonzero_u64
  ownership_record_revision: nonzero_u64
  source_owner_generation: blake3_256
  source_admission_policy_revision: u64
  status: ACTIVE | CUTOVER_PREPARED | FENCED | RETIRED
  cutover_receipt_ref: digest | null
```

`source_owner_generation` is the canonical digest of the namespace, owner system, installation incarnation, owner epoch, ownership-record revision, and status. It changes on restart-incarnation replacement, owner fencing, activation, retirement, or cutover; `source_admission_policy_revision` remains a separate policy axis. The wire fields `owner_system_id` and `source_owner_generation` are copied exactly from this record rather than reconstructed by an adapter.

Search is the sole authoritative mutable owner of identity and revision history for every namespace it admits. A client, research service, importer, or replacement Search installation may retain immutable references or imported snapshots, but MUST NOT mutate the same lineage concurrently. Ownership transfer requires a prepared identity mapping, source/view fence, compatibility verification, old-owner fencing, new-owner activation, and a cutover receipt.

The cross-provider cutover receipt has one exact wire shape:

```yaml
protocol: source.owner-cutover.v1

cutover:
  cutover_id:
  source_namespace_id:
  identity_mapping_digest:
  prepared_at:
  effective_at:

old_owner:
  owner_system_id:
  source_owner_generation_before_fence:
  fence_revision:
  final_source_view_ref:
  final_revision_set_digest:
  terminal_status: FENCED | RETIRED

new_owner:
  owner_system_id:
  source_owner_generation_after_activation:
  activation_revision:
  admitted_revision_set_digest:
  status: ACTIVE

validation:
  compatibility_receipt_refs: []
  integrity_receipt_refs: []
  unresolved_sources_and_reasons: []

authorization:
  old_owner_authorization_ref:
  new_owner_authorization_ref:
  issued_at:
```

Canonical `source.owner-cutover.v1` body SHA-256: `b659806e37a4bc60ea67b4416e35212f559213bbadb28618b7edcee686b9277e`. The digest is computed over the UTF-8 body inside the fence, excluding fence lines and the final line feed. A receipt is valid only when both owners authorize the same namespace and identity mapping, the old generation is already fenced, the new generation is active, final/admitted revision-set digests match, and every unresolved source is explicit. Failure before activation leaves no second active owner; abort or resume requires a new state-machine receipt. Unknown load-bearing fields or a mismatched generation, view, owner, or revision-set digest fail closed.

### S7.3 SourceIdentity, PathBinding and SourceRevision

```yaml
SourceIdentity:
  source_namespace_id: uuid
  source_id: uuid
  identity_kind: ntfs_file | git_blob_lineage | imported_object | admitted_virtual_snapshot
  stable_identity_components: opaque

PathBinding:
  binding_id: uuid
  source_id: uuid
  workspace_id: uuid
  display_path: string
  canonical_path_key: opaque
  first_seen_revision: revision_id
  last_seen_revision: revision_id | null

SourceRevision:
  revision_id: uuid
  source_id: uuid
  occurrence_sequence: u64
  content_digest: blake3_256
  byte_length: u64
  observed_at: timestamp
  acquisition_kind: filesystem | git_object | imported | admitted_ide_snapshot
  stability_receipt: digest
  object_residency_key_digest: blake3_256
```

A revert `A → B → A` creates three revision occurrences even though the first and third share a content digest. Hard links may have multiple PathBindings to one physical SourceIdentity. Paths are locators, not identity.

### S7.4 Corpus membership is separate from source identity

```yaml
SourceMembership:
  source_membership_id: uuid
  corpus_id: uuid
  source_id: uuid
  workspace_id: uuid
  role: source | test | documentation | generated | vendor | reference
  preparation_profile_id: string
  access_policy_binding_id: uuid
  retention_policy_id: string
  residency_policy_binding_id: uuid
  membership_revision: u64
```

`SourceIdentity` contains no corpus role, access policy or membership array.

### S7.5 Materialization, representation and projection membership

```yaml
Materialization:
  materialization_id: uuid
  source_revision_id: uuid
  materializer_profile_id: string
  canonical_object_digest: blake3_256
  object_residency_key_digest: blake3_256
  native_coordinate_map_digest: blake3_256
  loss_map_digest: blake3_256
  assurance_ceiling: exact_bytes | mapped_text | lossy_text | descriptive_only

Representation:
  representation_id: uuid
  materialization_id: uuid
  unitizer_profile_id: string
  enrichment_profile_ids: [string]
  unit_manifest_digest: blake3_256

ProjectionMembership:
  projection_membership_id: uuid
  source_membership_id: uuid
  representation_id: uuid
  access_partition_id: uuid
  scoring_partition_id: uuid
  projection_schema_id: string
```

Baseline rule: one `ProjectionMembership` represents exactly one `SourceMembership`. Shared parsing/CAS is allowed only under an equivalent complete residency key; shared retrieval points across memberships are not.

### S7.6 Unit occurrence

```yaml
UnitOccurrence:
  unit_id: uuid
  representation_id: uuid
  unit_kind: file | section | symbol | reference | test | doc | table | image_region
  ordinal: u64
  native_anchor: NativeAnchor
  structural_identity: opaque | null
  configuration_predicate: string | null
```

A unit is an occurrence in a specific representation. It is not assumed stable across arbitrary reparses.

`ScoringDocumentId` is an opaque digest-derived identity of `(source revision, representation, unit, projection profile set)` with membership excluded. It exists only to detect equivalent retrieval copies and deduplicate/fuse them safely.

### S7.7 Reference portfolios

```yaml
ReferencePortfolioRevision:
  portfolio_id: uuid
  portfolio_revision: u64
  display_name: string
  included_workspace_or_corpus_ids: [uuid]
  membership_precedence: [uuid]
  lineage_collapse_policy_id: string
  role_filters: [source, test, documentation, reference]
  access_policy_binding_id: uuid
```

“Other repositories” always means an explicit admitted portfolio revision. Search does not crawl arbitrary disks, clone repositories, or invoke an online research system implicitly. A client adapter may select `task_default`; standalone mode requires an explicit or configured portfolio. An empty portfolio yields `REFERENCE_SCOPE_EMPTY`.

---

### S7.8 SourceView and WorkspaceViewRevision

Every query resolves one explicit source view before planning. Search MUST NOT silently mix the working tree, Git index, a commit, an imported snapshot or retained history.

```yaml
SourceView:
  kind: working_tree_current | git_index | git_commit | imported_snapshot | retained_revision
  workspace_instance_id: uuid | null
  workspace_view_revision_ref: uuid | null
  git_commit_oid: string | null
  imported_snapshot_id: uuid | null
  retained_revision_id: uuid | null
```

For `working_tree_current`, precedence is:

```text
authenticated unsaved IDE buffer
  > confirmed saved worktree revision
  > selected published base representation.
```

The workspace view is itself versioned:

```yaml
WorkspaceViewRevision:
  workspace_instance_id: uuid
  root_filesystem_identity: object
  repository_lineage_id: uuid | null
  head_commit_and_branch: object | null
  git_index_identity: object | null
  inventory_revision: u64
  worktree_observation_cursor: u64
  authenticated_ide_overlay_revision: u64
  ignore_and_source_admission_policy_revision: u64
```

Branch switch, checkout, rebase, root rebinding, Git-index change or IDE-overlay change creates a new `WorkspaceViewRevision`. A compound query uses one revision across all branches. Drift requires replan or an explicit stale/incomplete result; Search never combines results from two view revisions as one coherent answer.

---

## S8. Projection membership isolation

### S8.1 Baseline physical rule

```text
1 SourceMembership
→ 1 ProjectionMembership
→ N Qdrant points carrying only that opaque ProjectionMembership identity
```

A file included in two corpora may reuse raw bytes and materialization, but receives two retrieval memberships. This avoids policy ambiguity and prevents a payload from revealing names or counts of other corpus memberships.

### S8.2 Payload non-disclosure

A point MUST NOT contain:

- corpus names;
- repository display names not required for authorized result projection;
- membership arrays;
- ACL subject lists;
- names of inaccessible portfolios;
- raw client principal, task, or canonical-state identifiers.

Authorized display metadata is resolved after retrieval from the control snapshot. Access and scoring partition identifiers are immutable: a policy/scoring-population change creates a new identifier and projection publication rather than mutating the meaning of an existing ID.

### S8.3 Query-time duplicate-membership routing

Physical duplication must not become scoring duplication. Before building a safe leg, `MembershipRouteCompiler` collapses authorized equivalent memberships to at most one ProjectionMembership per `ScoringDocumentId` in that IDF population. Selection follows the explicit portfolio membership precedence and query role. If materially different roles must remain, they execute as separate safe legs and are combined by rank-based fusion, never one duplicated IDF corpus.

The response may disclose multiple authorized provenance memberships only after retrieval and only for the current grant. Inaccessible alternatives do not influence the choice or response.

### S8.4 Future physical deduplication

Physical point deduplication across memberships is deferred. It requires a dedicated proof that ranking, IDF population, facets, counts, grouping, debug traces, timing and payload disclosure remain noninterfering under all access-policy changes.

---

## S9. Qdrant collection contract

### S9.1 Baseline topology

```yaml
QdrantBaseline:
  nodes: 1
  shard_number: 1
  replication_factor: 1
  write_consistency_factor: 1
  collection_generations: one_active_plus_migration_candidate
  network_binding: loopback_only
  authentication: generated_api_key
  strict_mode: required
```

`shard_number = 1` is part of scoring identity. Changing it requires a new qualified collection generation.

### S9.2 Exact build pin

The runtime configuration contains both:

- a capability floor required by the architecture;
- one exact qualified Qdrant artifact version and digest.

Automatic upgrades are prohibited. Admission requires a startup capability suite, not a version string alone.

### S9.3 Required capability probes

```yaml
QdrantCapabilityProbe:
  - authenticated loopback health
  - collection schema digest equality
  - strict-mode enforcement
  - signed integer range-filter behavior
  - missing-field behavior in must_not range filter
  - sparse-vector IDF modifier
  - independent idf.corpus filter
  - payload indexes for every eligibility predicate
  - wait=true mutation acknowledgement
  - strong write ordering support
  - exact count and point readback
  - one-shard configuration
```

Failure returns `QDRANT_CAPABILITY_MISMATCH`; Search remains direct/exact capable and MUST NOT silently degrade to unsafe indexed retrieval.

### S9.4 Named vectors

Baseline collection schema reserves versioned names:

```text
lex_code_v1       sparse, deterministic lexical code profile
lex_text_neutral_v1 sparse, language-neutral document profile
sem_text_<profile> optional dense vector
late_<profile>      optional multivector
```

A profile change that alters tokenization, hashing, weighting, dimensions, quantization or interpretation requires a new vector/profile identity. Incompatible payload/currentness changes require a new collection generation.

### S9.5 Minimal opaque point payload

```yaml
QdrantPointPayload:
  installation_incarnation_id: uuid
  collection_generation_id: uuid
  projection_membership_id: uuid
  access_partition_id: uuid
  scoring_partition_id: uuid
  source_id: uuid
  source_revision_id: uuid
  representation_id: uuid
  unit_id: uuid
  point_identity_digest_256: hex
  scoring_document_id: uuid
  projection_profile_set_id: keyword
  unit_kind: keyword
  modality: keyword
  language_or_format: keyword
  entity_kind: keyword | null
  normalized_symbol_key: keyword | null
  repository_lineage_id: uuid | null
  valid_from_epoch: signed_int64
  valid_until_epoch_exclusive: signed_int64 | absent
```

Every filterable field has an explicit payload index before strict mode admits the collection.

---

## S10. Epoch domain and validity filter

### S10.1 Epoch type

Qdrant integer payloads use a signed 64-bit domain. Therefore:

```rust
struct Epoch(i64);

// valid values
0 <= epoch && epoch < i64::MAX
```

`0` denotes an empty initial generation. First publication uses `1`. `u64::MAX`, `i64::MAX` sentinels and floating-point epochs are prohibited. Approaching exhaustion forces a new collection generation and resets its epoch domain; an exhausted generation never accepts another publication.

### S10.2 Open-ended validity

An active point omits `valid_until_epoch_exclusive`. No numeric infinity sentinel exists.

For visible epoch `E`, eligibility is:

```text
valid_from_epoch <= E
AND NOT(valid_until_epoch_exclusive <= E)
```

The second clause is encoded as a `must_not` range condition. A capability fixture MUST prove that a missing upper-bound field passes this condition. If the qualified Qdrant build does not satisfy the fixture, the collection is not admitted.

### S10.3 Full pre-candidate eligibility filter

Every retrieval and every IDF-corpus computation uses the same mandatory base filter:

```text
installation incarnation
AND collection generation
AND selected projection membership / safe partition
AND current access-policy generation
AND valid_from <= E
AND NOT(valid_until <= E)
AND NOT live deny/shadow/purge/abandoned fence
AND vector/profile applicability
```

Query-specific topical predicates are added after this base. Facets, counts, grouping, recommend/discover and debug candidate traces use the same base.

---

## S11. Collision-safe point identity

### S11.1 Canonical key

```yaml
ProjectionPointKey:
  schema_version: 1
  installation_incarnation_id: uuid
  collection_generation_id: uuid
  projection_membership_id: uuid
  representation_id: uuid
  unit_id: uuid
  projection_profile_set_id: string
  point_role: unit | relation | auxiliary
```

The key is encoded with canonical CBOR. Ad-hoc string concatenation is forbidden. A unit point carries the named-vector set required by its immutable `projection_profile_set_id`; lexical and dense vectors of one published unit are not represented as unrelated identities. Adding or replacing a required vector profile is a new projection publication, so old and new point generations can coexist until the epoch commit.

### S11.2 ID derivation and collision guard

1. Compute `D = BLAKE3-256(canonical_key_bytes)`.
2. Derive the Qdrant UUID from a namespace-separated 128-bit projection of `D`.
3. Store the full `D` as `point_identity_digest_256` in payload and the projection manifest.
4. Before any upsert that may address an existing UUID, retrieve the point and compare the full digest and canonical identity fields.
5. A mismatch returns `POINT_ID_COLLISION`, blocks the publication and never overwrites the existing point.

The architecture does not claim collisions are impossible; it makes them detectable and non-destructive.

### S11.3 Projection manifest

Each prepared publication has an immutable CAS manifest containing exact point UUIDs, full identity digests, unit IDs, expected vector names and payload digest. redb stores only the manifest reference and publication state.

Broad payload filters MUST NOT be used to close or compensate a generation when exact point IDs are available.

---

## S12. Lexical retrieval contract

### S12.1 Qdrant is not assumed to be the text encoder

The architecture requires Qdrant sparse retrieval and IDF behavior. It does not assume that a self-hosted Qdrant server can transform arbitrary text into the required sparse vector.

`LexicalEncoderPort` is explicit:

```rust
trait LexicalEncoderPort {
    fn profile(&self) -> LexicalProfileDescriptor;
    fn encode_document(&self, input: LexicalInput) -> SparseVector;
    fn encode_query(&self, input: LexicalInput) -> SparseVector;
    fn fixture_digest(&self) -> Digest;
}
```

A server-side encoder adapter may be admitted only after the startup fixture proves exact profile behavior. Otherwise Search uses a bundled deterministic local encoder that emits Qdrant sparse vectors. This encoder is not an inverted index and stores no searchable corpus.

### S12.2 Immutable lexical profiles

```yaml
LexicalProfile.code_v1:
  tokenizer_semantics: pinned_word_and_identifier_expansion
  lowercase: true
  unicode_normalization: NFC
  ascii_folding: false
  stopwords: []
  stemmer: none
  min_token_length: 1
  max_token_length: 128
  identifier_expansion:
    - raw_identifier
    - snake_case_parts
    - camel_and_pascal_parts
    - qualified_name_parts
    - path_segments
  weighting_parameters: pinned_by_fixture
  token_to_sparse_index: pinned_by_fixture

LexicalProfile.text_neutral_v1:
  tokenizer_semantics: pinned_multilingual_or_word_fallback
  lowercase: true
  unicode_normalization: NFC
  ascii_folding: false
  stopwords: []
  stemmer: none
  min_token_length: 1
  max_token_length: 128
  weighting_parameters: pinned_by_fixture
  token_to_sparse_index: pinned_by_fixture
```

No implicit English language, stopword or stemming default is allowed. Concrete numerical weighting values and golden vectors are committed in P06 after the official-implementation fixture is reproduced.

`LexicalProfileId` is the digest of all behavior that can change document/query compatibility:

```text
provider artifact and version/hash;
tokenizer and Unicode normalization;
identifier splitting and case rules;
stopwords and stemming;
vocabulary or term-index mapping;
collision strategy;
weighting and BM25 parameters;
Qdrant sparse modifier and schema;
document/query compatibility fixture digest.
```

P06 selects exactly one lexical provider path for a collection generation:

```text
A. capability-proven Qdrant server-side BM25 document/query inference; or
B. bundled deterministic LexicalEncoderPort emitting Qdrant sparse vectors.
```

Search MUST NOT switch automatically between A and B. A change creates a new lexical profile and collection-generation migration.

If sparse term hashing is used, the collision policy and collision corpus are explicit and measured. Collision-prone lexical matches can nominate candidates, but cannot establish exact identity, completeness or absence. Exact symbol/text planes remain collision-free and every emitted candidate is revalidated from the source revision.

### S12.3 Exact identifiers do not depend on BM25

Exact symbol names, qualified symbol keys, repository IDs, paths and entity kinds use keyword/indexed payload predicates and the exact/source planes. BM25 nominates lexical analogues; it is not the exact symbol oracle.

### S12.4 IDF population

The `idf.corpus` filter is derived from the same currentness, access and selected-scope base filter as retrieval. Staged, retired, denied, purged or inaccessible points MUST NOT influence score statistics.

If filtered IDF is unavailable or fails its noninterference fixture, lexical indexed mode for that partition is rejected rather than run with a global leaking IDF population.

---

## S13. Publication model

### S13.1 One active commit transaction

Preparation workers may read, parse, unitize and encode in parallel. The `PublicationCoordinator` permits at most one active Qdrant commit transaction globally.

Prepared changes are micro-batched. No later epoch is staged while an earlier epoch is unresolved. This removes avoidable inter-epoch dependency graphs and is the baseline simplicity/performance trade-off.

### S13.2 Immediate invalidation before preparation

When a saved source change, deletion, restrictive policy change or purge is confirmed, Search installs a control-level shadow/deny fence before asynchronous preparation. Queries admitted after the fence cannot see the old membership even though old Qdrant points remain physically present.

### S13.3 Publication state machine

```text
PREPARED
  → INTENT_DURABLE
  → NEW_POINTS_ACKNOWLEDGED
  → OLD_POINTS_CLOSED_ACKNOWLEDGED
  → READBACK_VERIFIED
  → CONTROL_COMMITTED
  → RECLAIMABLE

failure before CONTROL_COMMITTED:
  → COMPENSATING
  → ABORTED
or
  → INVALIDATION_ONLY_COMMITTED
or
  → PUBLICATION_BLOCKED
```

### S13.4 Commit algorithm

Let current committed epoch be `E`; next epoch is `N = E + 1`.

1. Verify owner epoch, source revisions, access bindings, purge state and prepared manifest.
2. Persist `PublicationIntent(N)` in redb.
3. Upsert exact new point IDs with `valid_from=N` and no upper bound.
4. Use Qdrant `wait=true` and strong ordering; read back exact IDs and payload/vector manifest.
5. Set `valid_until=N` on the exact old point-ID list from the previous manifest.
6. Use `wait=true`; read back every closed ID and exact count.
7. Prepare expected owner/source/membership/access/shadow/purge generation guards.
8. In one redb compare-and-swap transaction, verify every guard still equals the prepared value and only then:
   - set `VisibleEpoch = N`;
   - publish new manifest refs;
   - retire old manifest refs;
   - remove only the matching source shadows;
   - record the publication receipt.
9. Publish the resulting immutable in-memory control snapshot before acknowledging the publication to callers. A cache-publication failure is fail-closed and recovered from redb; it never causes an unsafe success response.
10. Reclaim retired points only after the pin watermark permits it.

Qdrant alias changes are not the linearization point.

### S13.5 Crash and unresolved intent

An uncommitted intent never changes `VisibleEpoch`. Recovery may:

- idempotently complete after exact readback;
- compensate exact staged/closed IDs and mark `ABORTED`;
- intentionally commit invalidation-only;
- enter `PUBLICATION_BLOCKED`.

`doctor publication abandon` may unblock only after an `AbandonedPublicationFence` excludes the entire affected projection membership or physical partition before candidate generation and IDF. The skipped epoch is never reused.

### S13.6 Startup verification

After unclean shutdown, indexed mode remains quarantined until Search verifies:

- active collection route and schema identity;
- latest committed publication receipt;
- absence or resolved state of an active intent;
- current deny/purge fences;
- Qdrant capability fixture.

Direct reads and exact scans may remain available with explicit degraded status.

### S13.7 Collection-generation migration

An incompatible schema, vector profile, shard topology or qualified Qdrant storage migration builds a new physical collection generation. It never mutates the meaning of the active collection in place.

```text
CANDIDATE_CREATED
→ BASE_BUILT_AT_CONTROL_REVISION_R0
→ CHANGE_LOG_CAUGHT_UP
→ FINAL_PUBLICATION_BARRIER
→ CANDIDATE_VALIDATED_AT_R1
→ REDB_ROUTE_SWITCH_COMMITTED
→ OLD_ROUTE_DRAINING
→ OLD_ROUTE_RECLAIMED
```

Preparation and catch-up occur while the old route serves queries. The final barrier pauses new publication commits only long enough to apply the final ordered delta and validate candidate currentness. The linearization point is one redb transaction switching `(collection_generation_id, visible_epoch, schema_identity)`; Qdrant aliases are optional operational labels. In-memory route pins keep the old collection until every admitted query/continuation releases it. A failed candidate is deleted without changing the active route. Live deny/purge fences apply throughout build and cutover.

---

## S14. Query snapshots, pins and reclamation

### S14.1 Read-only hot path

Short-query admission reads an immutable in-memory `ControlSnapshot` and performs no redb write.

```yaml
QuerySnapshotFence:
  installation_incarnation_id: uuid
  collection_generation_id: uuid
  visible_epoch: Epoch
  catalog_revision: u64
  membership_revision: u64
  reference_portfolio_revision: u64 | null
  access_policy_revision: u64
  shadow_fence_revision: u64
  purge_fence_revision: u64
  overlay_revision: u64
  observation_cursor_revision: u64
  observation_freshness: current_confirmed | observed_with_age | gap_detected | unknown
  source_view: object
  workspace_view_revision: object | null
  lexical_profile_ids: [string]
```

### S14.2 In-memory epoch and route pin

Each active indexed query acquires an RAII pin for `(collection_generation, visible_epoch)`. The pin lives through all retrieval legs, source readback and result projection.

No durable `QueryFenceLease` is written for an ordinary query.

### S14.3 Reclamation watermark

The reclaimer may delete retired points only when they are invisible to:

- all active in-memory epoch pins;
- bounded ephemeral continuation pins;
- explicit durable jobs that own a retained source snapshot.

A daemon crash ends short queries and their pins. Durable jobs do not rely on a process-local Qdrant pin after restart; they resume from persisted source/job checkpoints or replan.

### S14.4 Live security fences override snapshots

Restrictive access revocation and purge use one security linearization path:

```text
1. acquire the SecurityMutationBarrier for the affected security domain;
2. commit the new durable access/purge generation;
3. publish a new immutable LiveDenySnapshot;
4. invalidate affected grants, plans, handles and continuations;
5. acknowledge the mutation only after steps 2–4 are observable.
```

A request rechecks the latest live security state:

```text
at request admission;
before and after every scoring/IDF leg;
before source readback;
before result emission;
before handle expansion or continuation.
```

If durable policy commits but the new deny snapshot cannot be published safely, the affected domain enters `SECURITY_FAIL_CLOSED`; no indexed or source result is emitted until reconciliation.

If a revoked membership participated in a Qdrant scoring or IDF leg, Search discards the entire affected leg and replans under the latest grant; removing only forbidden candidates is insufficient because they may already have influenced rank statistics. If safe re-execution cannot complete within budget, the response reports `ACCESS_REVOKED`/`INCOMPLETE_COVERAGE` and exposes none of the contaminated ordering. Direct legs that provably never touched the revoked population may remain. Security and legal deletion are monotonic-deny concerns, not ordinary snapshot-isolation concerns.

---

## S15. Coherent source readback

### S15.1 Why current filesystem bytes are insufficient

A point at epoch `E` may refer to revision `A` while the filesystem already contains revision `B`. Reading `B` and citing it as `A` is forbidden.

### S15.2 Revision retention strategy

For each published representation Search must be able to reopen the exact revision by one of:

1. immutable Git object;
2. retained raw revision CAS object;
3. immutable imported object;
4. active authenticated BufferSnapshot.

Working-tree revisions needed by a currently visible epoch are retained in the raw revision CAS until all epoch pins and handle-retention requirements release them. CAS objects deduplicate by content digest.

### S15.3 Readback rule

Before a candidate is projected:

- open the exact `SourceRevision`;
- verify digest and byte length;
- resolve the unit anchor through its coordinate map;
- verify selected text/structure against unit digest;
- reject or replan on mismatch.

Qdrant payload snippets may be used only as non-authoritative previews and are not cited.

---

## S16. Source acquisition and change reconciliation

### S16.1 Watchers are hints

Filesystem notifications and the Windows USN journal accelerate detection but do not prove completeness. Search performs:

- startup reconciliation;
- resume/logon reconciliation;
- periodic bounded inventory sweeps;
- explicit user-requested reconcile;
- gap recovery after watcher overflow.

### S16.2 Query freshness preflight

For `current_workspace` intent, Search checks watcher/USN gap state and the relevant root observation cursor before indexed planning. If the cursor is not continuous, it reconciles within budget or reports `OBSERVATION_GAP`; it does not label the index current. Candidate validation also compares the live PathBinding head metadata with the published SourceRevision. A mismatch installs a shadow/reconciliation request and drops or directly rereads the candidate. Historical/frozen revision queries may deliberately use retained CAS/Git bytes instead.

A fast non-strict query may use an observed snapshot only when the response exposes observation age and freshness state. Exact negative proof never relies on this relaxed mode.

### S16.3 Stable no-execute read

A source read records identity/metadata before and after the read and verifies content digest. Unstable files retry within budget then become `SOURCE_UNSTABLE`.

Search MUST NOT execute:

- Git hooks or credential prompts;
- repository build scripts;
- Office macros;
- archive members;
- document remote resources;
- language-server build commands without a separate admitted provider policy.

### S16.4 Reparse points, links and path escape

Root policy is checked after final-handle resolution. Symlink/reparse traversal that escapes an admitted root is denied unless the target root is independently admitted. Cycles are detected. Case folding and Unicode normalization form a lookup key but the original display path is preserved.

### S16.5 Default exclusions

Baseline excludes VCS internals, known credential files, secret-key formats, build outputs, dependency caches, huge binaries and generated/vendor trees unless the corpus policy explicitly admits them. Query logs never compensate for unsafe ingestion.

### S16.6 SourceAdmissionPolicy and sensitivity

Root registration is necessary but not sufficient for ingestion. Every membership is evaluated under a versioned `SourceAdmissionPolicy` before materialization or indexing.

```yaml
SourceAdmissionPolicy:
  policy_revision: u64
  denied_system_locations: [opaque_rule_id]
  denied_filename_and_format_classes: [opaque_rule_id]
  secret_and_private_key_detectors: [profile_id]
  generated_vendor_and_binary_policy: object
  maximum_file_archive_and_materialization_limits: object
  sensitivity_classes: [public, project, confidential, secret_candidate]
  explicit_override_authority: object
  disclosure_and_logging_policy: object
```

OS/browser credential stores, private-key locations and known token files are deny-by-default. Potentially sensitive repository files require explicit corpus policy and a compatible grant `sensitivity_ceiling`. Detection does not copy the secret into logs or Qdrant.

The baseline Qdrant payload is content-minimized: opaque identities, validity metadata, profile IDs and small structural facets only. Raw source text, excerpts, absolute paths, corpus display names, secrets and query text are forbidden. Text and citations are produced only by governed source readback.

---

## S17. Materialization and enrichment

### S17.1 Baseline formats

P00–P15 require:

- raw text and source code;
- Git object reading;
- Rust structural enrichment.

PDF/Office/OCR/archive materialization is optional and does not block lexical/code product acceptance.

### S17.2 Provider neutrality

No document materializer implementation is selected by this architecture. Xberg, Kreuzberg or another provider may be admitted only through an ADR and qualification suite covering:

- exact package/version and license;
- native Windows deployment;
- no Python/Node production dependency unless explicitly accepted;
- PDF/OCR engine identity;
- no-execute behavior;
- coordinate/loss maps;
- archive bombs and resource limits;
- malformed-input isolation;
- replacement/uninstall behavior.

Unverified claims such as “Xberg v5+ uses PDFium” are not architectural facts.

### S17.3 Parser assurance

Tree-sitter yields tolerant syntax structure, not compiler truth. Provider assurance and configuration predicates travel with every structural fact. SCIP/LSP/compiler providers may later raise assurance through separate adapters.

---

## S18. Saved and unsaved overlay

```text
published Qdrant candidates
− every source membership shadowed by a saved or unsaved newer revision
+ direct exact/token/structural candidates from overlay bytes
→ fusion and validation
```

Saved overlay comes from confirmed filesystem revisions awaiting publication. Unsaved overlay exists only through an authenticated IDE buffer feed.

A saved overlay is a confirmed durable source revision awaiting publication and therefore follows the ordinary residency, retention, access, and purge contracts. An unsaved overlay is:

- bounded by bytes, sources, binding, authenticated editor snapshot, and TTL;
- memory-only and excluded from redb, CAS, Qdrant, backup, restore manifests, crash-dump collection, provider caches, telemetry payloads, evaluation corpora, and learning/training inputs;
- represented durably only by non-reconstructive fencing metadata such as digest, size, editor/session identity, and invalidation cursor;
- invalidated when the authenticated snapshot closes, is replaced, loses authorization, or exceeds TTL.

Only an explicit save or governed snapshot-admission operation may create a durable `SourceRevision` and residency receipt. A durable handle cannot point to unsaved bytes. If overlay qualification exceeds budget, Search returns an explicit coverage gap or performs invalidation-only; it never silently exposes stale base points or persists the buffer as a convenience.

---

## S19. Access compilation and noninterference

### S19.1 SearchReadGrant

A standalone or optional client binding supplies a signed/paired grant containing:

```yaml
SearchReadGrantClaims:
  grant_id: uuid
  installation_id: uuid
  installation_incarnation_id: uuid
  binding_id: uuid
  principal_opaque_id: opaque
  client_scope_ref: opaque
  scope_domain_id: opaque
  allowed_membership_ids: [uuid]
  allowed_corpus_or_portfolio_ids: [uuid]
  reference_portfolio_revision: u64 | null
  allowed_access_partitions: [uuid]
  allowed_modalities: [keyword]
  permitted_recipe_families: [string]
  maximum_budget_class: string
  sensitivity_ceiling: public | project | confidential | secret_candidate
  disclosure_ceiling: local_only | named_client | exportable
  source_read_permission: bool
  exact_scan_permission: bool
  issued_boot_id: opaque
  issued_at: timestamp
  expires_at: timestamp
  nonce: opaque
  revocation_generation: u64
```

Search validates the grant at request admission, intersects every requested scope with server-authoritative membership state, and revalidates it at the security checkpoints in S14.4. Agents cannot supply raw collection names, Qdrant filters, point IDs, partition IDs or access predicates. Ordinary reads do not write a durable lease to redb.

### S19.2 SearchTaskPlan

After grant validation and server-authoritative scope intersection, Search compiles one immutable execution plan:

```yaml
SearchTaskPlan:
  plan_id: uuid
  provider_protocol_version: string
  request_id: uuid
  recipe_and_normalized_request_digest: object
  grant_id_and_revocation_generation: object
  client_scope_ref_and_scope_domain_id: object
  source_view: SourceView
  workspace_view_revision_ref: opaque | null
  source_namespace_owner_generations: [object]
  selected_membership_ids: [uuid]
  reference_portfolio_revision: u64 | null
  access_policy_and_live_deny_generations: object
  visible_epoch_and_collection_route_revision: object
  provider_lexical_and_fusion_profiles: object
  overlay_snapshot_refs: [opaque]
  query_execution_budget: QueryExecutionBudget
  exactness_and_coverage_requirements: object
  state_dependencies: [object]
  plan_fingerprint: blake3_256
  created_at_and_expires_at: object
```

The plan is produced by Search, not accepted as a client-authored Qdrant plan. It contains no raw collection name, point ID, vendor filter, unrestricted path set, or authority-bearing client predicate. One plan binds one coherent source/workspace view and the exact source-owner, access, deny, publication, profile, and budget generations used to compile every leg. Any load-bearing drift forces revalidation, replan, or an explicit stale/incomplete result. `PlanFingerprint` is computed from the canonical serialization of these load-bearing fields.

### S19.3 Retrieval legs

A selected portfolio is compiled into one or more safe retrieval legs. Baseline uses one immutable corpus/scoring partition per leg. Multiple partitions may be grouped only when the control journal has a current `OverlapFreeRouteProof` showing that no equivalent `ScoringDocumentId` can enter the grouped IDF population twice and that access policy is identical. Search never sends an unbounded list of per-file membership IDs merely to repair overlap.

Cross-repository portfolios therefore execute a bounded set of per-partition legs and combine them with versioned rank-based fusion. The planner applies leg budgets, priority and continuation when the portfolio is large. Every leg has one coherent access/scoring population and an indexed Qdrant filter before candidate generation.

### S19.4 No post-filter-only security

Final result filtering is defense in depth. It is not sufficient because unauthorized content could already affect score, IDF, counts, diversity or traces.

---

## S20. Query recipes

The public Search provider surface is versioned and small:

```yaml
RecipeSet_v1:
  - locate@1
  - find_text@1
  - inspect_entity@1
  - compare_implementations@1
  - explore_entity@1
  - corpus_profile@1
  - corpus_delta@1
  - provenance@1
  - compile_exact_scan@1
  - execute_exact_scan@1
  - expand_handle@1
```

A client adapter may expose these recipes through existing client surfaces; Search does not require a new client-global tool name. The optional ELIOT adapter maps them through existing query/verification surfaces and does not add `eliot.search`.

### S20.1 Example natural-language path

User intent:

```text
Find parse_manifest in this repository and show how it is expected to work in other repositories.
```

The consuming client or adapter compiles:

```yaml
recipe: compare_implementations@1
subject:
  entity_name: parse_manifest
  current_scope: active_workspace
references:
  portfolio: task_default
comparison_axes:
  - interface
  - validation
  - errors
  - side_effects
  - tests
  - callers
  - documentation
```

Search resolves `task_default` to one immutable `ReferencePortfolioRevision`, then handles repository selection, exact/structural/lexical legs, lineage collapse, readback, and compact comparison. The consuming client owns interpretation and admission. Search never silently substitutes online or unregistered repositories.

---

## S21. Subject resolution and retrieval planning

### S21.1 Subject resolver

Resolution ladder:

1. explicit source handle or editor cursor;
2. qualified symbol key;
3. exact normalized name in current workspace;
4. signature/entity-kind compatibility;
5. structural and lexical candidates.

Ambiguous subjects return a bounded `SubjectAmbiguitySet`; Search does not silently choose among materially different definitions.

### S21.2 Retrieval ladder

```text
A. direct current overlay / exact source scan
B. exact Qdrant keyword predicates
C. structural provider facts
D. sparse lexical retrieval
E. optional dense semantic retrieval
F. optional rerank / late interaction
```

Later legs are additive and budgeted. A generative model is not part of the Search hot path.

### S21.3 Deterministic fusion

Search fuses only authorized leg outputs. Raw BM25 scores from different scoring/access partitions are never compared as if they shared one population. Within a safe leg, Qdrant may fuse compatible named-vector branches under a pinned fusion profile. Across legs, Search uses a versioned rank-based fusion profile (baseline weighted RRF), then applies:

- exact/entity-kind boosts;
- evidence-role quotas;
- repository-lineage diversity;
- fork/mirror/copy collapse;
- deterministic point-ID tie break;
- per-source and per-lineage caps.

Scores are query-local routing evidence, not durable facts. `FusionProfileId` is part of the query-plan and evaluation identity.

A `PlanFingerprint` hashes the normalized recipe, source view, workspace/reference revisions, grant/security generations, provider and lexical profiles, budgets and fusion profile. Equal fingerprints MUST produce the same leg graph and stable result ordering.

The baseline final tie-break is:

```text
assurance class
> evidence-role priority
> fused rank
> portfolio priority
> repository lineage identity
> source identity
> native coordinate
> projection point ID.
```

Raw scores from different populations never participate directly in this tie-break.

---

## S22. Cross-repository comparison

`compare_implementations@1` produces descriptive evidence, not a normative verdict.

```yaml
CrossRepositoryBehaviorSet:
  local_subject:
    definition: handle
    signature: observation
    callers: [handle]
    tests: [handle]
    documentation: [handle]
  comparable_implementations:
    - lineage_id: opaque
      match_basis: exact_name | normalized_name | signature | structural | lexical | semantic
      configuration_predicate: string | null
      evidence_roles: [definition, test, caller, documentation]
      behavior_signature: object
      exact_handles: [handle]
  comparison:
    shared_observations: [object]
    variants: [object]
    outliers: [object]
    locally_absent_observations: [object]
    conflicts: [object]
    unknowns: [object]
  coverage: object
  recommended_reading: [handle]
```

Five forks of one implementation count as one lineage for independent-evidence summaries. Tests and documentation are evidence roles, not automatic truth.

---

## S23. Candidate validation and result projection

### S23.1 Validation

Every selected candidate is validated against:

- query fence and latest live deny/purge fences;
- exact projection membership;
- source revision digest;
- unit/anchor digest;
- configuration predicate;
- provider assurance;
- current overlay shadow.

Invalid candidates are removed with a reason code. If removal materially changes coverage, Search replans within budget or returns an explicit gap.

A Qdrant point is never emitted as evidence by itself. Before emission Search must resolve the exact source/revision handle, recheck deny and purge state, reopen the fenced revision, verify its digest, validate `NativeAnchor`, and confirm the extractor/profile identity. If readback is impossible, the candidate may be reported only as `STALE`, `UNREADABLE` or `INCOMPLETE_COVERAGE`; it is not presented as a confirmed citation.

### S23.2 SearchCandidateSet

The generic provider response is a validated candidate product, not a belief or admission decision:

```yaml
SearchCandidateSet:
  request_id_and_plan_id: object
  plan_fingerprint: blake3_256
  source_view_and_workspace_view_revision_refs: object
  source_owner_access_deny_and_route_generations: object
  candidates:
    - candidate_id: opaque
      source_handle: SearchSourceHandle
      evidence_role_and_entity_kind: object
      assurance_freshness_and_validation_state: object
      ranking_trace: bounded_non_content_metadata
      reason_codes: [string]
  coverage:
    requested_and_executed_legs: object
    represented_memberships_and_source_lineages: object
    omitted_or_failed_legs_and_reasons: object
    observation_freshness_and_unknowns: object
    denominator_kind: candidate_scope | complete_scope | unknown
  continuation_handle: opaque | null
  result_validation_receipt_ref: digest
```

Every emitted candidate has passed S23.1 readback and live-fence validation. `denominator_kind = complete_scope` is available only from an executed S25 exact plan; ordinary top-k retrieval remains `candidate_scope` or `unknown` and cannot support an absence claim. The response exposes no raw Qdrant collection, filter, offset, point payload, or reusable authorization decision. A consuming client may cite or admit a candidate only after applying its own governance to the immutable source handle and coverage record.

### S23.3 Compact result budget

Default result cards include:

- answer-oriented navigation summary;
- 2–4 recommended exact source handles;
- material variants/conflicts;
- coverage, freshness and gaps;
- one continuation handle when useful.

Raw multi-repository dumps are prohibited by default.

---

## S24. Native coordinates and anchors

### S24.1 Canonical anchor variants

```yaml
NativeAnchor:
  TextBytes:
    content_digest: blake3_256
    byte_start_0: u64
    byte_end_exclusive_0: u64
  GitBlobBytes:
    repository_lineage_id: uuid
    commit_oid: string
    path_bytes: bytes
    byte_start_0: u64
    byte_end_exclusive_0: u64
  BufferRange:
    buffer_snapshot_id: uuid
    buffer_version: u64
    position_encoding: utf8_bytes | utf16_code_units | utf32_codepoints
    start_line_0: u64
    start_character_0: u64
    end_line_0: u64
    end_character_0: u64
  PdfRegion:
    source_revision_id: uuid
    page_1: u64
    coordinate_space: crop_box_points_after_rotation
    x0: float
    y0: float
    x1: float
    y1: float
  ArchiveMember:
    archive_revision_id: uuid
    member_path_bytes: bytes
    nested_anchor: NativeAnchor
```

### S24.2 Derived line/column display

Human-facing text lines are 1-based. Canonical text columns are UTF-8 byte offsets from the raw line start unless another encoding is explicitly named. LSP UTF-16 coordinates are adapter coordinates and MUST carry their encoding.

CRLF normalization, transcoding, OCR and parser transformations require an explicit coordinate map and loss map. Search does not fabricate raw-byte exactness when mapping is lossy.

---

## S25. Exact verification plane

### S25.1 Compile then execute

`compile_exact_scan@1` produces a frozen plan:

```yaml
ExactScanPlan:
  plan_id: uuid
  predicate:
    kind: literal | regex | qualified_symbol | structural_pattern | record_field
    engine_and_version: string
    serialized_form: object
    input_domain: raw_bytes | decoded_text | structural_ir
    worst_case_complexity_class: string
  denominator:
    source_revision_ids: [uuid]
    inventory_revision: u64
  inclusion_policy_digest: blake3_256
  unsaved_buffer_snapshot_ids: [uuid]
  completeness_requirements: object
```

`execute_exact_scan@1` reads every denominator item or reports why it could not.

### S25.2 Complete negative claim

`NoMatchInCompleteScope` requires:

- authoritative denominator from the source inventory;
- stable or retained exact revisions;
- every item readable;
- no timeout, cancellation or provider gap;
- exact predicate semantics;
- no scope change;
- an execution report consumable by a client verifier; the optional ELIOT adapter maps it to ELIOT verification.

It proves only the stated predicate. It does not prove absence of an arbitrary semantic analogue. Regex predicates use a pinned non-backtracking engine/profile with explicit size and time bounds; user patterns are never delegated to an unbounded backtracking runtime.

### S25.3 Live-scope drift

If a source changes after plan compilation and the old revision is not retained, execution returns `SCOPE_CHANGED_OR_REVISION_UNAVAILABLE`, never a complete negative result.

---

## S26. Handles and continuations

### S26.1 Ephemeral handles

Default result and continuation handles are:

- opaque random identifiers;
- binding scoped;
- authorization checked on every expansion;
- bounded by TTL and count;
- stored in memory;
- invalid after daemon restart.

They may reference unsaved buffers but never outlive the authenticated buffer snapshot.

### S26.2 Durable source handles

```yaml
SearchSourceHandle:
  handle_id_and_revision:
  source_namespace_id_and_owner_generation:
  source_revision_ref:
  source_view_and_workspace_view_revision_refs:
  native_anchor_and_excerpt_digest:
  materialization_profile_and_assurance_ceiling:
  object_residency_key_digest:
  retention_expiry_and_invalidation_refs:
```

A durable source handle may survive restart only when it identifies an immutable retained source revision and native anchor. It grants no access by itself; current authorization, owner generation, residency authorization, and purge fences are checked on every expansion. A handle cannot target an unsaved overlay.

Search handles are provider-local and are not canonical evidence for any consuming client. A client must snapshot, pin, or import load-bearing evidence under its own governance.

### S26.3 Continuation stability

Search does not expose raw Qdrant offsets or score cursors. A continuation either:

1. retains a bounded in-memory candidate window and epoch pin; or
2. re-executes the versioned plan under the stored fence and suppresses already-issued point IDs.

If the fence or retained data expires, Search returns `SNAPSHOT_EXPIRED` with an explicit refresh option. It does not silently continue against a newer corpus.

---

## S27. Qdrant process ownership and security

### S27.1 Sole supervisor

`eliot-searchd.exe` is the sole owner of qdrant.exe lifecycle and credentials. It:

- owns the data-root process lock;
- starts the exact qualified binary;
- binds it to loopback;
- supplies generated API credentials;
- applies filesystem ACLs;
- places the child in a Windows Job Object or equivalent lifecycle boundary;
- performs health/schema/capability admission;
- restarts with bounded backoff;
- quarantines after repeated or identity-mismatched failures;
- drains and stops it during controlled shutdown.

CLI, client adapters, and workers never connect directly to Qdrant.

### S27.2 Secret storage

The local API key is generated per installation incarnation and protected with OS user-bound secret storage. It is never written to logs, repository files or command-line arguments visible to other processes.

### S27.3 Strict mode and indexes

Strict mode is enabled only after all mandatory payload indexes exist. A request that would require an unindexed eligibility filter is rejected, not executed as an unbounded scan.

### S27.4 Orphan handling

Search never blindly attaches to a process or data directory merely because a port responds. It verifies executable identity, owner record, installation incarnation, collection route and API credential. Ambiguity yields quarantine.

---

## S28. Recovery, rebuild and backup

### S28.1 Rebuildability first

The index is a projection. Baseline disaster recovery is:

```text
preserve user configuration/source membership export
create new installation/collection generation
reacquire source revisions
rebuild projections
```

Independent Qdrant or redb snapshots are not automatically current.

### S28.2 Paired recovery manifest

Any optional accelerated backup must include a paired manifest binding:

- installation incarnation;
- redb checkpoint digest;
- Qdrant snapshot identity;
- collection schema and generation;
- committed visible epoch;
- latest publication receipt;
- purge-tombstone generation.

Restore always enters `RESTORE_PENDING_REVALIDATION` and revalidates external sources and access policy before serving indexed results.

### S28.3 Purge

Purge installs a live deny fence first, then removes projection points, manifests, unneeded CAS objects and handles. A receipt distinguishes:

- logical non-accessibility;
- index deletion;
- cache deletion;
- backup/snapshot status;
- physical secure-erasure limitations on modern storage.

Purge tombstones apply before reindex and restore so deleted material cannot resurrect.

### S28.4 CAS retention and mark-and-sweep

Reference counting alone is insufficient because crashes, shared memberships, query pins, durable handles, restore and purge can desynchronize counters. Search-owned CAS uses crash-safe mark-and-sweep.

Durable roots include:

```text
active projection/publication manifests;
publication and compensation intents;
retained SourceRevision leases;
durable source handles and client pin/import/export contracts;
recovery manifests;
retention and legal-hold records.
```

Active query, continuation and route pins are added to the protection set during a sweep. Sweep records its root generation, mark manifest and deletion receipt so interruption is resumable. Removing one membership never deletes bytes still reachable from another membership, active publication or handle.

Security/legal purge dominates ordinary retention. It installs `PurgeFence` before acknowledgement, revokes Search handles, blocks restore/reindex, deletes Search-owned Qdrant/CAS content as technically possible and retains only a non-content tombstone. Search sends a typed revocation event for evidence already imported by a client; it does not obtain authority to delete client-owned canonical evidence.

---

## S29. Runtime profiles

```yaml
Profile.DIRECT:
  processes: [eliot-searchd]
  capabilities: [inventory, exact_read, exact_scan, overlays]

Profile.LEXICAL:
  processes: [eliot-searchd, qdrant]
  capabilities: [DIRECT, keyword, sparse_bm25, recipes, compact_cards]

Profile.CODE:
  processes: [eliot-searchd, qdrant]
  capabilities: [LEXICAL, rust_structure, inspect, compare]

Profile.SEMANTIC_OPTIONAL:
  processes: [eliot-searchd, qdrant, model_worker_on_demand]
  capabilities: [CODE, dense, optional_rerank, optional_multivector]

Profile.DOCUMENT_OPTIONAL:
  processes: [eliot-searchd, qdrant, doc_worker_on_demand]
  capabilities: [LEXICAL, qualified_materializers]
```

Optional workers are absent or stopped when not needed.

### S29.1 Content-minimized observability

Default logs and metrics contain operation IDs, opaque capability/reason codes, counts, durations, resource use and collection/profile identities. They do not contain source bodies, unsaved buffers, raw query text, absolute paths, API keys or authorized corpus names. Privileged debug traces are explicit, binding-scoped, access-filtered, TTL-bounded and disabled by default. Crash-dump policy must treat daemon/worker memory as potentially source-bearing.

---

## S30. Resource governance and candidate SLOs

### S30.1 Resource priorities

Foreground query/readback outranks indexing, optimization and optional model work. Search pauses or throttles background preparation under user-defined CPU, memory, disk and GPU pressure.

### S30.2 Candidate SLOs, not claims

The implementation must measure before acceptance:

```text
warm exact/keyword navigation      p95 target ≤ 100 ms
warm single-scope lexical query    p95 target ≤ 200 ms
warm cross-repository comparison   p95 target ≤ 700 ms before source expansions
first useful progressive card      target ≤ 300 ms when local branch is ready
```

These are acceptance targets on the control corpus, not guaranteed facts.

### S30.3 QueryExecutionBudget and scheduler

Every request is admitted with a server-bounded execution budget:

```yaml
QueryExecutionBudget:
  priority_class: interactive | verification | background
  deadline_ms: u64
  max_scoring_legs: u32
  max_prefetch_candidates_per_leg: u32
  max_validated_candidates: u32
  max_source_read_bytes: u64
  max_exact_scan_items: u64
  max_exact_scan_bytes: u64
  max_materialized_result_bytes: u64
  max_cpu_ms: u64
  max_memory_bytes: u64
```

The daemon uses bounded queues, per-binding quotas and separate interactive/verification/background lanes. Cancellation propagates to Qdrant calls, source readback, exact scans and optional workers. Epoch/route pins are released on completion, cancellation or connection loss. Saturation returns `RESOURCE_EXHAUSTED` or a truthful partial result; it never creates an unbounded queue.

Read requests create no durable job or idempotency row. Mutating commands use a bounded redb idempotency table keyed by operation identity so reconnect/retry cannot create duplicate corpus registration, purge or publication.

---

## S31. Crate and dependency architecture

Recommended focused crates:

```text
search-contracts
search-domain
search-control-redb
search-source
search-prep
search-lexical
search-index-qdrant
search-query
search-runtime
search-eliot-adapter
search-eval
```

Binaries:

```text
eliot-searchd
eliot-search
eliot-search-model-worker        optional
eliot-search-doc-worker          optional
```

Dependency direction:

```text
contracts
  ↑ domain
  ↑ source / prep / lexical / query
  ↑ adapters: redb / qdrant / eliot
  ↑ runtime and binaries
```

Forbidden dependencies:

- contracts/domain on Qdrant, redb, client-system internals, or Windows APIs;
- Qdrant adapter on recipe meaning or client admission;
- any client adapter on Qdrant types;
- workers on redb ownership;
- query planner on raw database clients.

A capability cell becomes a separate crate only when it has a real dependency, replacement, test or context boundary.

---

## S32. Client integration contract

### S32.0 Local provider transport

On Windows, clients connect to a per-installation named pipe owned by `eliot-searchd`. Frames are `u32 little-endian byte_length` followed by UTF-8 JSON.

```yaml
ProviderEnvelope:
  protocol_major: u16
  protocol_minor: u16
  installation_incarnation_id: uuid
  binding_id: uuid
  connection_sequence: u64
  request_id: uuid
  message_kind: hello | request | progress | result | error | cancel | cancelled
  relative_deadline_ms: u64 | null
  body: object
```

The connection begins with a mutual-authenticated hello and major/minor capability negotiation. Baseline limits are explicit and versioned:

```text
maximum frame size: 8 MiB;
maximum concurrent in-flight requests per connection: 32;
monotonic connection sequence with replay/duplicate rejection;
idempotent cancellation;
request-relative deadlines;
no compression in the baseline;
no unbounded fragmented-message assembly.
```

Larger evidence is returned through bounded handle expansion, not oversized frames. Progress/result events are monotonically sequenced per request. Cancellation and disconnect release all request-local pins and resources. Non-Windows ports may map the same envelope to a Unix-domain socket without changing semantics.

### S32.1 Generic client boundary

```text
client question/context + explicit scope/view
→ typed SearchTaskPlan + SearchReadGrant
→ Search provider request
→ SearchCandidateSet / comparison / exact execution report
→ client interpretation, verification, and admission
```

Search returns candidates with coverage, freshness, provider assurance, source handles, and reason codes. It never returns a client memory disposition, canonical decision, task completion, or authorization decision. The adapter receives no raw Qdrant/redb access and cannot widen server-authoritative membership or policy state.

For an admitted source namespace, Search owns mutable source identity/revisions. A client receives immutable `SourceRevisionRef` values, result handles, and export receipts. A replacement provider or importer that must become the new mutable owner uses the source-owner cutover contract from S7.2.1; ordinary export is not a cutover.

### S32.2 SearchProviderCapabilityDescriptor

After authenticated binding, Search exposes a binding-filtered capability descriptor:

```yaml
SearchProviderCapabilityDescriptor:
  provider_protocol_version: string
  installation_id: uuid
  installation_incarnation_id: uuid
  data_root_identity: opaque
  owner_epoch: u64
  source_owner_generations: [object]
  supported_recipes: [string]
  available_profiles: [string]
  optional_provider_states: [object]
  visible_epoch: i64 | null
  collection_route_revision: u64
  access_policy_generation: u64
  source_inventory_revision: u64
  observation_freshness: object
  readiness_by_membership: [object]
  degraded_reason_codes: [string]
```

The descriptor contains only opaque memberships visible to the binding. A client uses it for planning and truthful coverage; availability never grants Search task, verification, admission, synthesis, or completion authority.

### S32.3 Optional ELIOT compatibility profile

The ELIOT adapter is a leaf package and maps the current external-provider boundary directly:

| ELIOT-side contract | Search mapping |
|---|---|
| `WorkScope` / disclosure policy | `client_scope_ref`, `scope_domain_id`, grant membership closure, and disclosure ceiling |
| `SourceView` / `WorkspaceViewRevision` | exact query view and coherent readback dependency |
| `StateFence` | request-bound dependency generations and source/view revisions |
| local provider capability pulse | `SearchProviderCapabilityDescriptor` |
| provider result | candidates, coverage, freshness, assurance, reason codes, and immutable source refs |
| canonical admission / finish | remains entirely inside ELIOT |

Compatibility invariants:

```text
Search is the sole mutable source owner for each admitted local namespace;
ELIOT stores immutable SourceRevisionRef values and governed influence records;
Search receives no canonical credentials, task authority, admission authority, or finish authority;
Search never returns an ELIOT memory disposition;
provider failure narrows coverage and never blocks unrelated ELIOT work;
no direct database access, shared credentials, or reverse write channel exists.
```

The adapter may map recipes through existing ELIOT query/verification surfaces. It does not add a new core `eliot.search` authority surface and does not import ELIOT internals into Search contracts/domain crates.

### S32.4 Optional Eliot Research normalized-bundle export

A separate leaf adapter may export a qualified durable Search materialization through the exact `eliotr.normalized.v1` manifest. Unsaved overlays are ineligible unless an explicit snapshot admission has created a durable `SourceRevision` and residency receipt:

```yaml
protocol: eliotr.normalized.v1

origin:
  owner_system_id:
  source_namespace_id:
  source_owner_generation:
  source_revision_ref:
  source_view_ref: exact_upstream_view_descriptor
  workspace_view_revision_ref: optional
  ownership_mode: federated_reference | immutable_import | ownership_cutover
  ownership_cutover_receipt_ref: required_for_ownership_cutover | absent_otherwise

source:
  logical_id:
  original_name:
  original_sha256:
  origin_location_class: local_only | cloud | external
  mime_type:

residency_and_disclosure:
  scope_domain_id:
  access_domain_id:
  confidentiality_domain_id:
  encryption_key_domain_id:
  retention_domain_id:
  erasure_domain_id:
  disclosure_ceiling:
  allowed_use:
  expiry:

normalization:
  analyzer:
  analyzer_version:
  profile:
  config_hash:
  created_at:

content:
  markdown: content.md
  markdown_sha256:
  structure: optional
  mappings: optional
  tables: optional
  coordinate_map_digest: optional
  loss_map_digest: optional

capabilities:
  text_ranges: boolean
  pages: boolean
  bounding_boxes: boolean
  tables: boolean
  figures: boolean

quality:
  state: high_fidelity | standard | degraded
  assurance_ceiling:
  warnings: []

export:
  purpose:
  receipt_ref:
```

Canonical `eliotr.normalized.v1` manifest-body SHA-256 (UTF-8; code fences and the final line feed excluded): `3a5f9fd2b254eebe574b2c4a28f9804df0da9df359e59ceee125fa7da90fef22`.

This is a wire manifest, not a shared implementation type. Unknown load-bearing fields fail closed; optional fields remain optional only where the protocol says so. Before export, Search reopens the exact retained source/materialization bytes, verifies their native BLAKE3 identities and lengths, and independently computes the protocol-required SHA-256 values; it never relabels an internal digest or hashes Qdrant payload text. The ordinary export creates an immutable Research import candidate. It grants no permission to mutate Search source history, transfers no source ownership, and permits no cross-domain CAS deduplication or key reuse. `ownership_cutover` is valid only after the separate S7.2.1 owner-transfer protocol has completed. The receipt MUST bind the old owner generation and source/view fence, the identity mapping, and the new owner generation and activation; the manifest records that completed transition but cannot authorize or perform it. The receipt field MUST be absent in every other ownership mode.

## S33. Standalone contract

Standalone CLI uses the same daemon, recipes, access compiler and result projector. It does not open redb or Qdrant directly. The local user binding mints a bounded standalone grant.

Managed and standalone owners cannot simultaneously own one data root. Mode transition requires drain, owner-epoch fence and restart.

---

## S34. Error and degradation model

Key reason codes:

```text
QDRANT_UNAVAILABLE
QDRANT_CAPABILITY_MISMATCH
COLLECTION_SCHEMA_MISMATCH
PUBLICATION_BLOCKED
PUBLICATION_READBACK_MISMATCH
POINT_ID_COLLISION
SOURCE_UNSTABLE
OBSERVATION_GAP
SOURCE_REVISION_UNAVAILABLE
SCOPE_CHANGED_OR_REVISION_UNAVAILABLE
REFERENCE_SCOPE_EMPTY
AMBIGUOUS_SUBJECT
UNSAVED_BUFFER_UNOBSERVED
UNSAVED_SNAPSHOT_NOT_ADMITTED
SOURCE_NAMESPACE_OWNERSHIP_CONFLICT
SOURCE_OWNER_CUTOVER_REQUIRED
RESIDENCY_DOMAIN_MISMATCH
CLIENT_ADAPTER_AUTHORITY_VIOLATION
ACCESS_REVOKED
PURGED
SNAPSHOT_EXPIRED
INDEX_GAP
INCOMPLETE_COVERAGE
MATERIALIZATION_LOSS
CONTROL_STORE_CORRUPT
RESTORE_PENDING_REVALIDATION
```

Degradation is visible. Direct exact capability may continue while indexed mode is blocked, but Search must not relabel direct results as complete lexical/semantic coverage.

---

## S35. Evaluation and Product Pulse

### S35.1 Required control corpus

The acceptance corpus includes:

- an actively edited local function;
- exact and renamed analogues in at least eight reference repositories;
- a same-name false positive;
- tests containing a decisive edge case;
- mutually exclusive configuration variants;
- a fork and mirror;
- nested repository/submodule;
- stale, unindexed and inaccessible repositories;
- saved and unsaved edits;
- watcher gap and resume reconciliation;
- publication crash at every failpoint;
- access revoke during query;
- purge/restore attempt;
- point-ID collision fixture;
- multilingual documentation and non-ASCII paths.

### S35.2 Baselines

```text
A  raw grep/read
B  Codebase Memory or current comparison tool
C  ELIOT Search
```

Measure:

- correct grounded action rate;
- recall of oracle definitions/tests/docs;
- false analogue rate;
- source reads and tokens consumed;
- time to first correct grounded action;
- stale/access leakage;
- p50/p95 latency;
- RAM, disk and background CPU;
- recovery correctness.

Semantic depth is not admitted until lexical/code Product Pulse passes.

---

## S36. Property and fault proofs

Mandatory properties:

```yaml
PropertySuite:
  - no point is current before VisibleEpoch commit
  - no retired point is reclaimed while an epoch pin can observe it
  - no query admission writes redb
  - no membership payload reveals another corpus membership
  - access revocation blocks emission and handle expansion immediately
  - missing valid_until behaves as open-ended only under the admitted fixture
  - point-ID collision never overwrites an existing point
  - exact negative proof fails on unreadable or changed denominator item
  - unsaved bytes never persist without explicit snapshot admission
  - journal restore cannot attach to a mismatched collection generation
  - aborted publication cannot hide old points in a later epoch without fence or compensation
  - stale Qdrant text cannot be cited without exact source readback
```

Failpoints cover every transition in S13.3, redb commit boundaries, Qdrant restarts, process termination, sleep/resume and disk-full behavior.

---

## S37. Delivery gates

### Gate G0 — Contract

Identity, membership, epoch, anchors, recipes, reason codes and dependency directions compile and pass pure property tests.

### Gate G1 — Direct

One daemon owns the root; source inventory, exact reads/scans, revision CAS and no-execute policy work without Qdrant.

### Gate G2 — Lexical

Qualified Qdrant, lexical encoder fixtures, filtered IDF, strict mode, point manifests and publication fault tests pass.

### Gate G3 — Code

Rust subject resolution, structure, callers/tests/docs and cross-repository comparison pass the control corpus.

### Gate G4 — Generic Client Edge and Optional Compatibility Profiles

Binding, grants, compact cards, handle expansion, evidence snapshot/pin/import, and verification reports work through a generic client fixture. The optional ELIOT mapping and Eliot Research export profiles pass their own fixtures only when enabled; neither is required for standalone baseline acceptance.

### Gate G5 — Product acceptance

Search beats or materially complements baselines without stale/access leakage and within resource budgets.

### Gate G6 — Optional depth

Dense, rerank, multivector and document providers are individually admitted only after measured benefit exceeds cost and risk.

---

## S38. Superseded or rejected decisions

```text
REJECTED  SQLite/FTS5 as a baseline or fallback search database
REJECTED  shared Qdrant point with corpus-membership arrays
REJECTED  SourceIdentity carrying access/corpus policy
REJECTED  u64::MAX or numeric infinity in Qdrant payload
REJECTED  unchecked stable_hash as a point ID
REJECTED  durable QueryFenceLease for every request
REJECTED  broad filtered closure when exact old point IDs exist
REJECTED  assuming self-hosted server text inference without a fixture
REJECTED  implicit English BM25 analyzer defaults
REJECTED  unqualified Xberg/PDFium production selection
REJECTED  simultaneous unresolved future publication epochs
REJECTED  source readback from whatever bytes currently occupy a path
REJECTED  direct client access to Qdrant
REJECTED  client-specific `ADMITTED_STRONG` or any other memory/admission disposition in Search core results
COMPATIBILITY ONLY  legacy `eliot.query` / `eliot.verify` labels at an optional client adapter; generic Search contracts remain canonical
```

---

## S39. Implementation readiness verdict

The architecture is coherent enough to begin **P00 contract implementation**, not to claim product completion.

```yaml
StaticVerdict:
  architecture_boundary: coherent_candidate
  qdrant_only_retrieval: enforced
  micromodule_ownership: defined
  crash_model: specified
  access_noninterference: specified
  codex_entry_point: P00
  runtime_evidence: absent
  performance_evidence: absent
  security_execution_evidence: absent
  product_acceptance: not_accepted
  external_repository_alignment: complete
```

<!-- END EMBEDDED ARCHITECTURE -->

---

# Part II — Codex Handoff

# ELIOT Search Codex Handoff 2.7
## Implementation plan for the embedded Architecture 8.4

**Normative architecture:** Part I of this same master document, between the architecture boundary markers.  
**Expected embedded Architecture SHA-256:** `ae4c18ccff256ce4d5fdf91dfd9041236ff6f332b611bae3bd748c2da8ac6a1c`  
**Entry point:** P00 only.  
**Previous handoff 2.6 and all earlier separate Search handoff files:** superseded.

---

## H0. Contract challenge

Before changing code, Codex MUST:

1. load this single master document;
2. extract the exact bytes between `BEGIN EMBEDDED ARCHITECTURE` and `END EMBEDDED ARCHITECTURE` markers, excluding the marker lines;
3. compute SHA-256 and compare with `ae4c18ccff256ce4d5fdf91dfd9041236ff6f332b611bae3bd748c2da8ac6a1c`;
4. enumerate S0–S39 headings and confirm S7.8, S16.6, S28.4, S30.3 and S32.2;
5. stop with `CONTRACT_CHALLENGE` on mismatch, duplicate architecture body or missing section.

Codex MUST NOT silently substitute a separate Architecture/Handoff file or an earlier draft.

---

## H1. Repository outcome

### H1.1 Workspace

```text
eliot-search/
├─ Cargo.toml
├─ Cargo.lock
├─ rust-toolchain.toml
├─ deny.toml
├─ crates/
│  ├─ search-contracts/
│  ├─ search-domain/
│  ├─ search-control-redb/
│  ├─ search-source/
│  ├─ search-prep/
│  ├─ search-lexical/
│  ├─ search-index-qdrant/
│  ├─ search-query/
│  ├─ search-runtime/
│  ├─ search-eliot-adapter/
│  └─ search-eval/
├─ bins/
│  ├─ eliot-searchd/
│  ├─ eliot-search/
│  ├─ eliot-search-model-worker/       # optional feature/profile
│  └─ eliot-search-doc-worker/         # optional feature/profile
├─ fixtures/
├─ migrations/
├─ docs/adr/
└─ tests/
```

A crate is created only when its dependency/test/replacement boundary is real. Empty forwarding crates are forbidden.

### H1.2 Dependency direction

```text
search-contracts
      ↑
search-domain
      ↑
source / prep / lexical / query
      ↑
redb-adapter / qdrant-adapter / eliot-adapter
      ↑
search-runtime and binaries
```

CI MUST run a dependency-direction test. Vendor types cannot cross public ports.

---

## H2. Toolchain, licenses and unsafe code

### H2.1 Toolchain

Pin one stable Rust toolchain in `rust-toolchain.toml`; commit `Cargo.lock`; use edition 2024 only if every selected dependency and CI runner passes on Windows.

### H2.2 Dependency policy

```toml
# deny.toml intent
[licenses]
allow = ["MIT", "Apache-2.0", "BSD-2-Clause", "Unicode-3.0", "ISC", "Zlib"]
confidence-threshold = 0.93

[bans]
multiple-versions = "warn"
wildcards = "deny"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
```

Every git dependency requires an ADR, exact revision and license proof.

### H2.3 Unsafe policy

`search-contracts`, `search-domain`, `search-query` and `search-control-redb` use `#![forbid(unsafe_code)]`. Native parser/OS adapter exceptions require an isolated module, safety comment and Miri or equivalent boundary tests where applicable.

### H2.4 Production runtime exclusions

Python and Node are not baseline runtime dependencies. A future document provider may change this only through an explicit ADR and packaging/security qualification.

---

## H3. Public contracts to implement first

### H3.1 Newtypes

```rust
pub struct InstallationId(pub Uuid);
pub struct InstallationIncarnationId(pub Uuid);
pub struct CollectionGenerationId(pub Uuid);
pub struct OwnerEpoch(pub NonZeroU64);
pub struct Epoch(pub i64); // 0 <= value < i64::MAX
pub struct CorpusId(pub Uuid);
pub struct ReferencePortfolioId(pub Uuid);
pub struct PortfolioRevision(pub u64);
pub struct SourceNamespaceId(pub Uuid);
pub struct SourceOwnerGeneration(pub NonZeroU64);
pub struct SourceId(pub Uuid);
pub struct SourceMembershipId(pub Uuid);
pub struct ProjectionMembershipId(pub Uuid);
pub struct SourceRevisionId(pub Uuid);
pub struct RepresentationId(pub Uuid);
pub struct UnitId(pub Uuid);
pub struct AccessPartitionId(pub Uuid);
pub struct ScoringPartitionId(pub Uuid);
pub struct ScoringDocumentId(pub Uuid);
pub struct ProjectionProfileSetId(pub String);
pub struct FusionProfileId(pub String);
pub struct ScopeDomainId(pub Uuid);
pub struct AccessDomainId(pub Uuid);
pub struct ConfidentialityDomainId(pub Uuid);
pub struct EncryptionKeyDomainId(pub Uuid);
pub struct RetentionDomainId(pub Uuid);
pub struct ErasureDomainId(pub Uuid);
pub struct ObjectResidencyKeyDigest(pub [u8; 32]);
pub struct PointIdentityDigest(pub [u8; 32]);
```

No raw string/UUID substitution at domain boundaries.

### H3.2 Epoch validation

```rust
impl Epoch {
    pub fn new(value: i64) -> Result<Self, ContractError> {
        if value < 0 || value == i64::MAX {
            return Err(ContractError::EpochOutOfRange);
        }
        Ok(Self(value))
    }

    pub fn checked_next(self) -> Result<Self, ContractError> {
        self.0.checked_add(1)
            .filter(|v| *v < i64::MAX)
            .map(Self)
            .ok_or(ContractError::EpochExhausted)
    }
}
```

Forbidden literals/tests: `u64::MAX`, `i64::MAX` as an active upper-bound payload, JSON floating epochs.

### H3.3 Membership contracts

Implement exact architecture objects:

- `SourceNamespaceOwnership`;
- `SourceIdentity` with no membership/policy fields;
- `PathBinding`;
- `SourceRevision` occurrence;
- `SourceMembership` with one corpus and one policy binding;
- `Materialization`;
- `Representation`;
- `ProjectionMembership` with one SourceMembership;
- `UnitOccurrence`;
- `SearchObjectResidencyKey` and `SourceResidencyProfileRef`.

Compile-fail or serialization tests MUST reject a `corpus_ids: Vec<_>` or membership array in Qdrant point payload.

### H3.4 Recipe set

```text
locate@1
find_text@1
inspect_entity@1
compare_implementations@1
explore_entity@1
corpus_profile@1
corpus_delta@1
provenance@1
compile_exact_scan@1
execute_exact_scan@1
expand_handle@1
```

Architecture and protocol generated schemas MUST expose exactly this set for v1.

### H3.5 Additional load-bearing contracts

P00 defines schema-only forms for:

```text
SourceView;
WorkspaceViewRevision;
SourceNamespaceOwnership;
SearchObjectResidencyKey;
SearchReadGrantClaims;
SearchTaskPlan;
SearchCandidateSet;
QueryExecutionBudget;
PlanFingerprint;
SourceAdmissionPolicy;
ProviderEnvelope;
SearchProviderCapabilityDescriptor;
SecurityMutationBarrier state and LiveDenySnapshot identity.
```

The P00 types contain no Qdrant, redb, Windows, or client-vendor types. Unknown fields follow the negotiated protocol-version rule; silently ignored security, scope or budget fields are forbidden.

---

## H4. Ports

```rust
pub trait ControlJournalPort { /* bounded control records only */ }
pub trait SourceInventoryPort { /* roots, bindings, revisions, memberships */ }
pub trait SourceRevisionStorePort { /* immutable bytes and manifests */ }
pub trait SourceOwnershipPort { /* namespace owner/cutover/fencing */ }
pub trait ResidencyPolicyPort { /* complete residency closure and copy/re-encrypt transitions */ }
pub trait MaterializerPort { /* optional format adapters */ }
pub trait UnitizerPort { /* deterministic unit occurrences */ }
pub trait CodeEnricherPort { /* definitions/references/provider assurance */ }
pub trait LexicalEncoderPort { /* document/query -> sparse vector */ }
pub trait SearchIndexPort { /* schema, upsert, close, query, readback */ }
pub trait AccessCompilerPort { /* grant -> safe legs */ }
pub trait OverlayPort { /* saved/unsaved transient state */ }
pub trait ExactScannerPort { /* compile/execute exact plan */ }
pub trait HandleStorePort { /* ephemeral/durable handle classes */ }
pub trait ClockPort {}
pub trait ProcessSupervisorPort { /* qdrant child lifecycle */ }
```

Ports use Search contracts, never Qdrant/redb/client-system types.

---

## H5. redb schema boundary

### H5.1 Required tables

```text
meta
installation
collection_route
source_roots
source_identities
path_bindings
source_revisions
source_memberships
materializations
representations
projection_memberships
scoring_route_proofs
publication_intents
publication_receipts
shadow_fences
deny_fences
purge_tombstones
abandoned_publication_fences
watcher_cursors
durable_jobs
cas_refs
```

Large point lists and source bodies are not stored in redb. Projection manifests live in the filesystem CAS and are referenced by digest.

### H5.2 Snapshot cache

After each committed control transaction, runtime constructs an immutable `Arc<ControlSnapshot>` and atomically publishes it. Query admission clones the Arc and writes nothing.

Test `hot_query_does_not_mutate_redb` compares transaction/write counters before and after 10,000 queries.

### H5.3 Restore rule

A journal whose incarnation/route does not exactly match the Qdrant collection is quarantined. Recovery creates a new collection generation; it does not infer current state from points.

---

## H6. CAS and source revisions

### H6.1 Layout

```text
cas/
  <residency-key-digest>/raw/<digest-prefix>/<digest>
  <residency-key-digest>/materialized/<digest-prefix>/<digest>
  <residency-key-digest>/maps/<digest-prefix>/<digest>
  <residency-key-digest>/manifests/<digest-prefix>/<digest>
```

Writes use temporary file + fsync + atomic rename. The complete residency-key digest and content digest are checked after reopen. A matching content digest under a different residency key is a different object and cannot reuse ciphertext or key material.

### H6.2 Retention

Raw revision CAS remains reachable while any visible epoch, retained handle or durable job requires it. GC is crash-safe mark-and-sweep, not refcount-only deletion. The mark root set includes active publications, publication/compensation intents, retained SourceRevision leases, durable handles, recovery manifests, retention/legal holds and client pin/import/export contracts. Active query/continuation pins are added for the sweep window. Purge fences override ordinary retention.

### H6.3 No-execute test corpus

Fixtures include symlink/reparse escape, Git hooks, macro documents, archive bombs, malformed encodings, huge files and files changing during read.

---

## H7. Point identity implementation

### H7.1 Canonical encoding

Use canonical CBOR with an explicitly versioned struct. Add golden bytes and digest fixtures. `serde_json` stringification is forbidden for ID derivation.

### H7.2 UUID derivation

Implement namespace separation and a 128-bit UUID projection from BLAKE3-256. Store full digest in payload and manifest. The canonical key addresses one projection-unit point and its immutable `projection_profile_set_id`, not one independent point per named vector.

### H7.3 Pre-upsert collision check

For every batch:

1. retrieve any existing target UUIDs;
2. compare full digest and identity fields;
3. abort with `POINT_ID_COLLISION` on mismatch;
4. never call upsert for a mismatched UUID.

A deterministic test injects two canonical keys through a fake truncated-ID adapter and proves no overwrite.

---

## H8. Qdrant supervisor and bridge

### H8.1 Ownership

Only `eliot-searchd` starts and authenticates qdrant.exe. The production bridge uses the official pinned Rust `qdrant-client` gRPC API for data operations; HTTP may be used only for a capability endpoint not exposed by the client. Server and client versions are qualified together. The CLI and tests use daemon ports/fakes unless a Qdrant integration fixture is explicitly selected.

### H8.2 Qualified artifact

Configuration contains:

```yaml
qdrant:
  capability_floor: "1.19.0"
  qualified_version: "SET_BY_P05_ADR"
  executable_sha256: "SET_BY_P05_ADR"
  bind_host: "127.0.0.1"
  strict_mode_required: true
  shard_number: 1
  replication_factor: 1
```

P05 resolves the exact qualified patch release from official artifacts and records its digest. “Latest” is forbidden.

### H8.3 Startup fixture

The bridge creates a disposable collection and proves:

- signed i64 range behavior;
- missing `valid_until` under `must_not lte`;
- required sparse modifier and filtered IDF;
- strict-mode rejection of unindexed filters;
- `wait=true` mutation/readback;
- exact count;
- one-shard schema.

Only then may it admit the production route.

### H8.4 Process security

- loopback only;
- generated API key in OS-bound secret store;
- data directory user ACL;
- no dashboard exposure without auth;
- Windows Job Object or tested equivalent;
- executable path/hash and PID recorded;
- bounded restart/circuit breaker;
- no credentials in argv or logs.

---

## H9. Lexical encoder

### H9.1 Select exactly one baseline provider

Implement `LexicalEncoderPort`. P06 selects exactly one provider for the collection generation:

```text
Qdrant server-side BM25 inference after capability fixture; or
bundled deterministic local sparse encoder.
```

A Qdrant document/text inference adapter must pass the same document/query golden fixture. Runtime fallback or automatic switching between providers is forbidden; provider/profile change requires a new collection generation.

### H9.2 Golden profiles

Commit fixtures for:

- `snake_case`, camelCase, PascalCase and qualified names;
- Unicode identifiers and paths;
- no implicit stopword removal;
- no stemming;
- code comments vs symbol tokens;
- text-neutral multilingual examples;
- document/query sparse vector equality against the qualified reference implementation.

The concrete `k`, `b`, expected average length, tokenizer and token-index mapping become immutable `LexicalProfileDescriptor` fields.

### H9.3 Exact branches

Qualified names and exact symbols use payload keyword indexes/direct structural data. BM25 is not used to prove exact identity.

---

## H10. Qdrant point schema

### H10.1 Payload

Use Architecture S9.5 exactly. Do not add display corpus names, arrays of memberships, ACL subject lists, or raw client identifiers.

### H10.2 Payload indexes

Before strict mode admission, create indexes for every field in the eligibility filter and common exact predicates. A generated schema digest covers field type, index options and vector profiles.

### H10.3 Active filter builder

One pure builder creates both retrieval and IDF base filters:

```text
valid_from <= E
AND NOT(valid_until <= E)
AND incarnation/generation/membership/access/profile
AND NOT live fences
```

Property tests compare both filter ASTs and reject divergence.

---

## H11. Publication coordinator

### H11.1 Concurrency

Preparation is parallel. Qdrant commit is a single globally serialized actor. At most one `PublicationIntent` can be active.

### H11.2 Exact manifests

Use exact point ID lists from old/new manifests for upsert verification, closure and compensation. Broad source/membership payload updates are forbidden on correctness paths.

### H11.3 Required calls

All publication mutations use `wait=true` and strong ordering. After each phase, retrieve/count exact IDs and verify payload/vector digest.

### H11.4 Commit point

Only a redb compare-and-swap transaction that verifies owner, source, membership, access, shadow and purge generations and then sets `VisibleEpoch=N` is the Search currentness linearization point. Add a failpoint between external recheck and transaction entry to prove the guard closes the race.

### H11.5 Failpoints

```text
fp_after_intent
fp_mid_new_upsert
fp_after_new_ack
fp_mid_old_close
fp_after_old_ack
fp_after_readback
fp_after_external_recheck_before_control_tx
fp_before_control_commit
fp_after_control_commit
fp_before_shadow_release
fp_during_reclaim
```

Each failpoint runs process-kill, reopen and invariant checks.

### H11.6 Operator recovery

Implement:

```text
eliot-search doctor publication inspect
eliot-search doctor publication compensate
eliot-search doctor publication invalidate-only
eliot-search doctor publication abandon
```

`abandon` requires a verified membership/partition exclusion fence before future epochs can commit.

---

## H12. Query pinning and continuations

### H12.1 EpochPinRegistry

In-memory registry keyed by collection generation and visible epoch. Query RAII guards live through source readback. Reclaimer unit tests prove it cannot delete points visible to a pin.

### H12.2 No ordinary durable lease

Delete or prohibit `QueryFenceLease` writes on normal request admission. Durable writes are allowed only for explicit durable jobs/continuations with separate quotas and schemas.

### H12.3 Continuation classes

```rust
pub enum ContinuationDurability {
    EphemeralInMemory,
    DurableReplanCheckpoint,
}
```

No raw Qdrant offset/cursor is public. Snapshot expiry is explicit.

---

## H13. Anchors and readback

### H13.1 Coordinate tests

Fixtures cover:

- UTF-8 multibyte text;
- CRLF and LF;
- non-UTF-8 source with declared decoding;
- LSP UTF-16 conversion;
- Tree-sitter byte ranges;
- Git blob paths with non-UTF-8 bytes where supported;
- PDF page/bbox mapping only in optional provider tests.

### H13.2 Source readback

A candidate is emitted only after exact revision reopen and digest/anchor validation. If the old working-tree bytes were not retained, return `SOURCE_REVISION_UNAVAILABLE` and replan or degrade.

---

## H14. Access, revocation and leakage tests

### H14.1 One membership per point

A fixture places the same file in two corpora with different policies. Assert:

- two ProjectionMemberships;
- no membership arrays;
- shared CAS allowed;
- unauthorized corpus name/count absent from payload and response;
- filtered IDF and ranking unchanged when inaccessible corpus content changes;
- when both memberships are authorized, equivalent copies never share one IDF leg unless an `OverlapFreeRouteProof` is valid; otherwise they run as separate legs and deduplicate by ScoringDocumentId after rank fusion.

### H14.2 Live revocation

Implement the S14.4 security linearization path: durable generation commit, immutable `LiveDenySnapshot`, affected grant/plan/handle/continuation invalidation, then acknowledgement. Recheck deny/purge at admission, before/after every scoring/IDF leg, before source readback, before emission and before expansion/continuation. If a revoked population influenced one scoring or `idf.corpus` leg, discard and re-execute that entire leg under the new grant; candidate-only filtering fails the test. Failure to publish a live deny snapshot after durable commit enters `SECURITY_FAIL_CLOSED`.

### H14.3 Facets and traces

Counts, grouping, facets, recommend/discover and debug traces all use the same eligibility base filter. Add negative tests for each surface.

---

## H15. Query service and result cards

The query service accepts a recipe request plus `SearchReadGrantClaims`, compiles the server-owned `SearchTaskPlan`, and returns `SearchCandidateSet`, comparison output, or an exact execution report. Contract tests reject client-authored vendor plans and verify that every result binds the same plan fingerprint, source view, owner/security generations, and coverage semantics.

### H15.1 Planner budgets

Every recipe receives a server-bounded `QueryExecutionBudget` with deadline, scoring-leg, prefetch, validated-candidate, source-read, exact-scan, result-bytes, CPU and memory ceilings. The daemon enforces bounded queues and per-binding quotas across interactive, verification and background lanes. Later legs are additive and cancellable; cancellation/disconnect releases all request-local pins. Saturation returns `RESOURCE_EXHAUSTED` or a truthful partial result.

### H15.2 Recipes

Implement in this order:

1. `locate@1`, `find_text@1`;
2. `inspect_entity@1`, `expand_handle@1`;
3. `compare_implementations@1`, `explore_entity@1`;
4. profile/delta/provenance;
5. exact compile/execute.

### H15.3 Determinism

Equal normalized recipe, SourceView, WorkspaceViewRevision, grant/security generations, provider profiles, budgets and fusion profile yield the same `PlanFingerprint`, leg graph and stable output ordering. Implement the Architecture tie-break exactly; raw scores from different populations never cross the rank-fusion boundary.

### H15.4 Compactness

Golden response tests enforce default source-handle and byte budgets. A result cannot contain raw full files or unbounded chunk arrays.

---

## H16. Client provider edge and optional compatibility profiles

### H16.1 Protocol

Windows named pipe with mutual-authenticated hello, major/minor negotiation and the exact `ProviderEnvelope` from S32.0. Use `u32` little-endian length prefix, UTF-8 JSON, 8 MiB default frame cap, at most 32 in-flight requests, monotonic connection sequence, replay rejection, idempotent cancel, relative deadlines and no baseline compression or unbounded fragment assembly. No second framing.

### H16.2 Binding

Pairing credential, installation/incarnation identity, boot identity, binding ID and `SearchReadGrantClaims` are checked before requests. Named-pipe ACL alone is insufficient. Client scope is intersected with server-authoritative memberships; raw Qdrant filters/collections/point IDs are never accepted.

### H16.3 No authority leak

Search response contracts contain no client admission dispositions. An adapter translates typed plans/results but does not read or write client canonical storage. The optional ELIOT adapter additionally proves the S32.3 mappings.

### H16.4 Durable evidence edge

Search handle → client snapshot/pin/import → client-governed evidence. Search never writes client canonical data.

### H16.5 Capability descriptor

After binding, return the exact `SearchProviderCapabilityDescriptor` from S32.2, filtered to opaque memberships visible to the binding. Descriptor availability is planning evidence only and never grants Search task, verification, synthesis, admission, or completion authority.

### H16.6 Optional normalized-bundle export

When enabled, the Eliot Research leaf adapter emits the exact `eliotr.normalized.v1` archive and validates the exact `source.owner-cutover.v1` receipt when ownership mode is `ownership_cutover`. It reopens retained bytes, verifies native identities and lengths, computes the wire SHA-256 values independently, rejects unknown load-bearing fields, and never exports an unsaved overlay without explicit snapshot admission. The adapter is absent from the standalone core dependency graph and is not required for baseline acceptance.

---

## H17. PR delivery graph

### P00 — Contract freeze

**Depends on:** architecture hash only.  
**Delivers:** workspace, contracts/newtypes, recipe set, reason codes, SourceView/WorkspaceViewRevision, `SourceNamespaceOwnership`, `SourceOwnerCutoverReceipt`, `SearchObjectResidencyKey`, `SearchTaskPlan`, `SearchCandidateSet`, grant/budget/envelope/capability/admission-policy schemas, invariants as tests, dependency and license policy.  
**Exit proof:**

```text
cargo fmt --check
cargo check --workspace
cargo test -p search-contracts -p search-domain
cargo deny check
contract_hash_test
recipe_set_exact_test
forbidden_epoch_sentinel_test
```

### P01 — Runtime owner shell

**Depends on:** P00.  
**Delivers:** daemon/CLI framing, data-root lock, owner epoch, standalone mode, clean shutdown.  
**Exit proof:** second-owner denial, crash/reopen owner test, CLI never opens stores.

### P02 — Bounded redb journal

**Depends on:** P01.  
**Delivers:** migrations, tables H5.1, atomic snapshot cache, corruption/quarantine behavior.  
**Exit proof:** migration fixtures, power-loss reopen, `hot_query_does_not_mutate_redb`.

### P03 — Source registry and direct exact spine

**Depends on:** P02.  
**Delivers:** roots, identities, PathBindings, memberships, versioned ReferencePortfolios, SourceView/WorkspaceViewRevision resolution, SourceAdmissionPolicy, stable reads, inventory, exact read/find, and the source-namespace owner state machine (`ACTIVE → CUTOVER_PREPARED → FENCED → RETIRED`) with prepare/fence/activate receipts.  
**Exit proof:** rename/hardlink/case/reparse/no-execute and unstable-read fixtures; deterministic wire-facing source-owner generation; generation change on fence, activation, or incarnation replacement; exact `source.owner-cutover.v1` schema/hash fixture; concurrent dual-writer denial; old-owner fencing before new-owner activation; mismatched owner/generation/view/revision-set, missing, stale, or partially authorized cutover-receipt rejection.

### P04 — Revision CAS and anchors

**Depends on:** P03.  
**Delivers:** raw revision retention, complete `SearchObjectResidencyKey`, residency-aware immutable paths, explicit copy/re-encrypt transitions, mark-and-sweep root model, manifests, coordinate contracts/conversions, and source readback.  
**Exit proof:** UTF-8/CRLF/LSP/Git and stale-path readback tests; key equality only under complete domain equivalence; cross-domain co-residency, physical deduplication, ciphertext reuse, and encryption-key reuse denied.

### P05 — Managed Qdrant process and capability gate

**Depends on:** P01–P04.  
**Delivers:** exact qualified artifact ADR, supervisor, API secret, strict mode, disposable capability suite.  
**Exit proof:** process identity, loopback/auth, schema mismatch, missing-field/range/IDF fixture.

### P06 — Lexical encoder and collection schema

**Depends on:** P05.  
**Delivers:** one explicitly selected lexical provider path, code/text-neutral profiles, golden document/query sparse vectors, collision profile, payload indexes, point identity and manifests.  
**Exit proof:** official-reference fixtures, collision injection, no implicit language defaults, no membership arrays.

### P07 — Epoch publication and recovery

**Depends on:** P02, P04, P06.  
**Delivers:** serialized coordinator, shadows, `wait=true` strong writes, exact closure/readback, pins/reclaimer, doctor commands.  
**Exit proof:** every H11.5 kill point, transactional generation-guard race, no staged visibility, no pinned reclamation, blocked/abandon recovery.

### P08 — Lexical query recipes and compact cards

**Depends on:** P07.  
**Delivers:** locate/find_text, QueryExecutionBudget scheduler, server-owned SearchTaskPlan compilation, validated SearchCandidateSet output, PlanFingerprint, bounded per-partition leg compiler, overlap-free grouping proofs, duplicate-membership routing by ScoringDocumentId, filtered IDF, deterministic cross-leg RRF, readback, ephemeral handles, evaluation baseline harness.  
**Exit proof:** access noninterference, deterministic cards, raw grep/read baseline captured.

### P09 — Change feeds and overlays

**Depends on:** P03, P07, P08.  
**Delivers:** watcher/USN hints, cursor continuity and observation-freshness state, strict current-workspace preflight, reconciliation, saved overlay, and authenticated unsaved buffers with an exhaustive ephemeral-content fence.  
**Exit proof:** overflow/resume, observation-gap currentness denial, selected-candidate live-head mismatch, deletion shadow, and proof that unsaved bytes do not enter CAS, redb, Qdrant, logs, telemetry payloads, backups, crash dumps, provider caches, evaluation corpora, or learning/training inputs; overlay budget degradation.

### P10 — Rust structural profile

**Depends on:** P04, P09.  
**Delivers:** Tree-sitter Rust definitions/references/tests/docs, provider assurance, configuration predicates.  
**Exit proof:** malformed code, cfg variants, non-UTF-8 rejection/mapping, no compiler-truth overclaim.

### P11 — Inspect and compare implementations

**Depends on:** P08, P10.  
**Delivers:** subject resolver, ambiguity sets, analogue ladder, lineage collapse, behavior matrix.  
**Exit proof:** renamed true analogue, false same-name, fork/mirror and decisive test fixtures.

### P12 — Exact scan and proof reports

**Depends on:** P03, P04, P09.  
**Delivers:** compile/execute split, frozen denominator, revision drift handling, report contract.  
**Exit proof:** complete literal negative, safe non-backtracking regex fixtures, raw-bytes vs decoded-text semantics, unreadable source, changed scope, cancellation and semantic-overclaim tests.

### P13 — Access, handles, purge and restore boundaries

**Depends on:** P07–P12.  
**Delivers:** SecurityMutationBarrier/LiveDenySnapshot linearization, durable source handles, CAS mark-and-sweep execution, purge receipts/tombstones, restore revalidation.  
**Exit proof:** revocation during query, handle denial, purge non-resurrection, mismatched restore quarantine.

### P14 — Generic Client Adapter Edge and Optional Compatibility Profiles

**Depends on:** P03, P04, P08, P09, P11–P13.  
**Delivers:** ProviderEnvelope flow control, protocol negotiation, pairing, full grant validation, capability descriptor, generic evidence pin edge, the disabled-by-default ELIOT profile, and the disabled-by-default Eliot Research `eliotr.normalized.v1` export profile.  
**Exit proof:** no canonical DB access or reverse authority; generic client request → server-owned SearchTaskPlan → SearchCandidateSet round trip with plan/view/generation/coverage binding; end-to-end compare query through a generic client fixture; optional ELIOT mapping fixture when enabled; exact Search↔Research normalized-bundle round trip, native-BLAKE3-to-wire-SHA-256 readback, unknown-field fail-closed behavior, ownership-mode validation, and exact `source.owner-cutover.v1` receipt validation when the Research export profile is enabled.

### P15 — Product Pulse and Windows qualification

**Depends on:** P14.  
**Delivers:** control corpus, A/B/C evaluation, latency/resource/fault/security report, source-admission/secret fixtures, protocol-flow-control stress, content-minimized telemetry and privileged-debug leakage audit.  
**Exit proof:** all S35 fixtures, acceptance report with raw evidence.  
**Gate:** no optional semantic/document work before acceptance decision.

### P16 — Optional semantic depth

**Depends on:** accepted P15 and ADR.  
**Delivers:** local model worker, versioned dense profile, measured fusion/rerank.  
**Exit proof:** material gain over lexical/code at acceptable cost; uninstall returns to P15 behavior.

### P17 — Optional document materializer

**Depends on:** accepted P15 and provider ADR.  
**Delivers:** isolated doc worker, selected provider, coordinate/loss maps.  
**Exit proof:** Windows packaging, no-execute/resource/fuzz suite, provider removal test.

### P18 — Optional advanced scale profile

**Depends on:** measured one-shard bottleneck.  
**Delivers:** the measured scale change plus the Architecture S13.7 collection-generation build/catch-up/final-barrier/redb-route cutover.  
**Exit proof:** kill tests at every migration state, old-route pin drainage, failed-candidate discard, scoring/currentness/noninterference equivalence and rollback plan.

---

## H18. Forbidden shortcuts

Codex MUST NOT:

- add SQLite, FTS5, Tantivy, Zoekt or another vector store;
- use redb for search text, postings or ranked retrieval;
- expose Qdrant clients to CLI/client adapters/workers;
- put corpus policy into SourceIdentity;
- put multiple memberships in one point;
- admit equivalent duplicate memberships into one IDF/scoring leg;
- emit an unbounded per-file membership filter instead of bounded partition legs;
- sanitize a rank leg after restrictive revoke instead of discarding/replanning it;
- create unrelated point identities for each named vector of one projection-unit publication;
- interpret “other repositories” without an explicit ReferencePortfolioRevision;
- use `u64::MAX`/`i64::MAX` as Qdrant infinity;
- derive point IDs from unchecked `stable_hash`;
- write a durable query lease for ordinary requests;
- close old points with a broad filter when a manifest exists;
- rely on `wait=false` on publication paths;
- mutate an incompatible active collection schema/profile in place or treat a Qdrant alias as the Search commit point;
- assume server BM25 text inference;
- accept implicit English stemming/stopwords;
- cite current path bytes as an older revision;
- commit VisibleEpoch after a non-transactional source/access generation check;
- call a workspace current while its observation cursor has an unresolved gap;
- reclaim an epoch with active pins;
- expose raw Qdrant offsets or point IDs as public handles;
- invent another client framing or bypass the named-pipe daemon protocol;
- log source bodies, unsaved buffers, raw query text, secrets or absolute paths by default;
- execute user regex with an unbounded backtracking engine;
- claim Xberg/PDFium or another provider before an ADR;
- treat a path, HEAD or nearest repository as an implicit SourceView;
- acknowledge restrictive policy before the corresponding LiveDenySnapshot and invalidations are active;
- accept raw Qdrant filters, collection names or point IDs from a client;
- auto-switch lexical providers inside one collection generation;
- use refcount-only CAS deletion as the correctness mechanism;
- create unbounded query queues, source reads, exact scans or transport reassembly;
- store source text, excerpts, absolute paths, secrets or query text in baseline Qdrant payload;
- omit transport replay, cancellation or in-flight limits;
- add `eliot.search` or give Search ELIOT authority;
- use global digest-only CAS paths or deduplicate/reuse ciphertext across unequal residency domains;
- persist unsaved buffers through CAS, redb, Qdrant, logs, telemetry, backups, crash dumps, provider caches, evaluation corpora, or learning/training inputs;
- let two providers mutate one source namespace without a fenced cutover receipt;
- treat `ownership_mode: ownership_cutover` as authority when the separately completed cutover receipt is absent, stale, or incompatible;
- start optional model/document work before P15 acceptance.

---

## H19. Definition of Done for baseline

Baseline is done only when:

```yaml
BaselineDoD:
  contracts: green
  direct_exact: green
  revision_readback: green
  qdrant_capability_gate: green
  lexical_profile_fixtures: green
  publication_fault_matrix: green
  access_noninterference: green
  security_revoke_linearization: green
  source_admission_and_secret_exclusion: green
  query_budget_and_backpressure: green
  provider_protocol_flow_control: green
  cas_mark_and_sweep: green
  saved_unsaved_overlay: green
  rust_structural_profile: green
  compare_implementations: green
  exact_proof_reports: green
  generic_client_provider_edge: green
  product_pulse: accepted
  optional_eliot_adapter: not_required
  optional_eliot_research_export_adapter: not_required
  optional_models: not_required
  optional_documents: not_required
```

A green unit-test suite without P15 evidence is not product acceptance.

---

## H20. First Codex assignment

```text
Implement P00 only.

Read this single master document and verify the embedded Architecture 8.4 hash
`ae4c18ccff256ce4d5fdf91dfd9041236ff6f332b611bae3bd748c2da8ac6a1c` first. Create the Rust workspace, contract/domain newtypes,
recipe enum, reason-code enum, SourceView/WorkspaceViewRevision,
SourceNamespaceOwnership, SourceOwnerCutoverReceipt, SearchObjectResidencyKey, SearchReadGrantClaims,
SearchTaskPlan, SearchCandidateSet, QueryExecutionBudget, SourceAdmissionPolicy, ProviderEnvelope,
SearchProviderCapabilityDescriptor, invariant tests,
dependency-direction checks, deny.toml and CI skeleton. Do not add Qdrant,
redb, filesystem watchers, Tree-sitter, model dependencies or runtime
implementation in P00.

Return:
1. changed file list;
2. architecture hash check;
3. contract decisions and exact deviations, if any;
4. command outputs for P00 exit proof;
5. blockers requiring an ADR.
```


---

# Part III — Consolidated Audit

# Consolidated audit of ELIOT Search 8.2 → 8.4

## Verdict

Architecture 8.2/8.3 and Handoff 2.3/2.4 had the correct overall boundary—Qdrant as the only retrieval database and redb as the technical journal—but were not yet safe for direct Codex handoff. Additional defects were found in currentness, membership isolation, point identity, source readback, and hot-path lifecycle. The 2.7 alignment revision also closes exact Search↔Research wire-schema, optional-adapter acceptance, and delivery-graph and public-contract defects A-58–A-63, in addition to A-51–A-57.

The single normative master document contains:

- Architecture 8.4;
- Codex Handoff 2.7;
- this consolidated audit and validation appendix.

## Confirmed defects and fixes

| ID | Severity | Defect in 8.2/2.3 | Fix in 8.3–8.4 / 2.4–2.7 |
|---|---:|---|---|
| A-01 | Blocker | `u64::MAX` was used or implied as the upper bound for Qdrant integer payload | `Epoch(i64)`; an open interval is represented by absent `valid_until`; capability fixture fixes filter semantics |
| A-02 | Major | Unverified wording `Xberg v5+ / PDFium` became a production dependency | Document materializer is provider-neutral again; selection only through ADR and Windows, no-execute, and loss-map qualification |
| A-03 | Blocker | Source identity was conflated with corpus policy and membership | `SourceIdentity`, `PathBinding`, `SourceMembership`, and `ProjectionMembership` are separated |
| A-04 | Blocker | Undefined `stable_hash` could address and overwrite another point identity | canonical CBOR + BLAKE3-256 + UUID projection + full digest collision check before upsert |
| A-05 | Major | A normal query created a durable `QueryFenceLease` in redb | hot path is read-only; `Arc<ControlSnapshot>` + in-memory RAII epoch pin |
| A-06 | Blocker | A shared point with a membership array exposed other corpora and complicated ACLs | baseline: one SourceMembership → one ProjectionMembership → separate points; shared CAS only |
| A-07 | Blocker | Qdrant was incorrectly treated as a guaranteed self-hosted text-to-BM25 encoder | explicit `LexicalEncoderPort`; server inference only after fixture, otherwise a local deterministic sparse encoder |
| A-08 | Major | BM25 could inherit implicit English stemming, stopwords, or a default profile | versioned code and text-neutral profiles, empty stopwords, no stemmer, golden vectors |
| A-09 | Blocker | Publication did not require strict acknowledgement and readback of every mutation | `wait=true`, strong ordering, exact ID readback and count, then the redb linearization point |
| A-10 | Major | Strict mode and required payload indexes were not a complete admission contract | disposable capability suite and schema digest before production-route admission |
| A-11 | Major | The owner of `qdrant.exe` was not fully defined | sole supervisor is `searchd`; loopback, API key, ACL, Job Object, identity check, quarantine |
| A-12 | Major | Ephemeral candidates, continuations, and durable evidence handles were conflated | three distinct lifetime and retention semantics; a Search handle is provider-local, not client canonical evidence |
| A-13 | Blocker | “Exact coordinates” did not define bytes, UTF-16, CRLF, or PDF coordinate basis | closed `NativeAnchor` enum, raw digest, and explicit conversion and loss maps |
| A-14 | Blocker | Removing QueryFenceLease left no mechanism preventing deletion of old points during a query | in-memory `EpochPinRegistry` and reclamation watermark |
| A-15 | Blocker | A query at epoch E could reread a changed path and report revision B as A | immutable Git object or retained raw-revision CAS; mandatory digest readback |
| A-16 | Blocker | Restoring old redb beside newer Qdrant could reaccept incompatible currentness | installation incarnation + collection-generation binding; mismatch → quarantine or new generation |
| A-17 | Major | Several unresolved future epochs could create cascading publication deadlock | preparation may be parallel, but only one active global Qdrant commit; micro-batching |
| A-18 | Blocker | Snapshot policy could return content after a restrictive revocation | live deny and purge fences are rechecked before emission or handle expansion and override the snapshot |
| A-19 | Major | Broad payload update during closing or compensation could affect staged or new points | exact old and new point-ID manifests in CAS; broad closure prohibited |
| A-20 | Major | Raw Qdrant pagination cursor was not stable after optimization or cleanup | server-side opaque continuation; bounded candidate window or fenced re-execution |
| A-21 | Major | The live scope of an exact negative scan could change during execution | frozen SourceRevision denominator; drift or unavailable revision makes proof incomplete |
| A-22 | Major | Query and IDF filters could diverge | one pure eligibility-filter builder; AST-equivalence property test |
| A-23 | Major | Staged, retired, or inaccessible documents could affect IDF statistics | identical access, currentness, and scoring-partition filter for `idf.corpus` |
| A-24 | Major | Control journal risked becoming per-point/search store | exact point lists moved to immutable CAS manifests; redb keeps references/state only |
| A-25 | Major | Independent Qdrant and redb backups could declare an inconsistent restore current | rebuild-first baseline; optional paired recovery manifest and `RESTORE_PENDING_REVALIDATION` |
| A-26 | Major | Watcher semantics risked claiming completeness | watcher and USN are hints only; startup, resume, and periodic reconciliation plus gap recovery |
| A-27 | Major | Path was used too close to identity | physical SourceIdentity + PathBinding and history; hardlink, rename, and case tests |
| A-28 | Major | Publication cleanup could enter the correctness path | old points remain logically hidden; physical reclamation only after commit and pin watermark |
| A-29 | Major | Point identity was tied to one vector profile, splitting one unit into disconnected lexical and dense points | one projection-unit point carries an immutable named-vector profile set; changing it publishes a new generation |
| A-30 | Major | “Other repositories” lacked a reproducible scope contract | versioned `ReferencePortfolioRevision`; no implicit clone, web access, or arbitrary disk scan |
| A-31 | Major | Raw BM25 scores from different access or scoring populations could be treated as comparable | safe scoring per leg + versioned weighted RRF across legs; raw scores never cross populations |
| A-32 | Major | One source admitted through several memberships could enter the IDF corpus twice | canonical membership route: at most one equivalent membership per ScoringDocumentId within a leg |
| A-33 | Blocker | Mid-query revocation could leave ranking already computed from a forbidden corpus | discard and replan the entire affected scoring or IDF leg; candidate-only filtering prohibited |
| A-34 | Major | Canonical membership routing could require a huge per-file filter and become impractical | baseline per-corpus or scoring-partition legs; grouping only with persisted overlap-free proof; bounded cross-leg RRF |
| A-35 | Major | Observability did not explicitly prohibit source or query leakage | content-minimized logs by default; privileged access-filtered TTL debugging only |
| A-36 | Major | Provider framing remained an implementation choice and could diverge across clients | fixed Windows named pipe + u32-LE length-prefixed UTF-8 JSON envelope |
| A-37 | Major | Exact regex could permit catastrophic backtracking or mix byte and text semantics | pinned non-backtracking engine, explicit input domain, and complexity and budget contract |
| A-38 | Blocker | Source or access could change between external recheck and redb VisibleEpoch commit | compare-and-swap generation guards inside the linearization transaction |
| A-39 | Blocker | A watcher gap could label an observed catalog as actually current | observation-freshness axis, strict current-workspace preflight, and live-head validation |
| A-40 | Blocker | A collection candidate was mentioned without a safe schema or profile migration cutover | build + ordered catch-up + final barrier + redb route switch + old-route pins |

## Deliberate simplifications

1. One active Qdrant publication commit instead of several future epochs.
2. One shard and one node in the baseline.
3. Duplicate retrieval points for different memberships instead of unsafe physical deduplication.
4. Baseline code, text, and Rust; documents and models after Product Pulse.
5. Rebuild-first disaster recovery instead of a complex mandatory paired-backup system.
6. A local deterministic lexical encoder is allowed as a preprocessing adapter, but not as an index or database.

## Still unproved

- exact qualified Qdrant patch build and its Windows fault behavior;
- real filtered-IDF noninterference;
- selected BM25 numeric parameters and identifier quality;
- Tree-sitter Rust recall on a representative versioned repository corpus;
- p95 latency, RAM and disk use, and optimizer contention;
- correctness at all NTFS kill and fault points;
- actual reduction of source reads and tokens for Codex;
- safety of any optional document or model provider;
- product acceptance through standalone clients, plus optional adapter acceptance when enabled.


## Additional defects A-41–A-50 closed in 8.4/2.5

| ID | Severity | Defect | Fix |
|---|---:|---|---|
| A-41 | Blocker | Restrictive revocation could be durable while an executing query continued on an old in-memory access snapshot | `SecurityMutationBarrier` → durable generation → `LiveDenySnapshot` → invalidation → ACK; repeated checks at every scoring, readback, and emission boundary; `SECURITY_FAIL_CLOSED` when the deny snapshot cannot be published |
| A-42 | Blocker | A grant was not fully bound to installation, WorkScope, portfolio revision, recipe, budget, and sensitivity ceiling | complete `SearchReadGrantClaims`; server-authoritative membership intersection; client-supplied raw Qdrant filters and IDs prohibited |
| A-43 | Blocker | The word `current` did not distinguish worktree, Git index, commit, imported snapshot, and retained revision | explicit `SourceView` and `WorkspaceViewRevision`; one view fence for every branch of a compound query |
| A-44 | Major | Query limits were abstract; unbounded queues, readback, and exact-scan risks remained | `QueryExecutionBudget`, bounded queues, per-binding quotas, priority lanes, cancellation, and `RESOURCE_EXHAUSTED`; durable idempotency for mutations only |
| A-45 | Blocker | Architecture allowed implicit lexical-provider selection or switching and an incomplete sparse-collision contract | exactly one provider path per collection generation; complete `LexicalProfileId`; collision policy and fixture; exact plane remains collision-free |
| A-46 | Major | Equal inputs did not guarantee the same plan or result; evidence emission could rely on a Qdrant point | `PlanFingerprint`, stable tie-break, source, revision, anchor, and profile readback before emission |
| A-47 | Blocker | CAS cleanup could rely on reference counts and delete shared, pinned, or recovery bytes | crash-safe mark-and-sweep with durable root generation, active ephemeral pins, and purge precedence |
| A-48 | Major | Named-pipe framing omitted negotiation, replay, cancellation, in-flight, and fragmentation limits | exact `ProviderEnvelope`, mutual hello, 8 MiB cap, 32 in-flight, monotonic sequence, replay rejection, idempotent cancel, no baseline compression |
| A-49 | Major | Clients lacked a typed provider readiness and capability view | binding-filtered `SearchProviderCapabilityDescriptor`; used for planning and coverage, creates no authority |
| A-50 | Blocker | An explicit root could index credentials, private keys, and secrets; Qdrant payload policy was too weak | versioned `SourceAdmissionPolicy`, deny-by-default sensitive system locations, sensitivity ceiling, content-minimized opaque payload, and governed source readback |

## Claude review D-1–D-7 — status in the unified document

| Review item | Resolution in master |
|---|---|
| D-1 stale first assignment | H0/H20 refer only to the embedded Architecture 8.4 and its section digest |
| D-2 recipe-name drift | One exact RecipeSet_v1 is used by Architecture and Handoff |
| D-3 global publication deadlock | Global `PUBLICATION_BLOCKED`, inspect/compensate/abandon operator path and no epoch reuse remain required |
| D-4 dependency policy loss | `deny.toml`, native runtime rule and provider-neutral document ADR remain in P00/P17 |
| D-5 Qdrant minimum | Exact qualified 1.19.x line and capability probe remain required before route admission |
| D-6 search-eval owner | Baseline harness is delivered with first lexical recipes and executed at Product Pulse |
| D-7 IDF/shards | Baseline one node/one shard; shard change is incompatible collection-generation migration |

## External-repository alignment fixes A-51–A-63

| ID | Severity | Defect | Closure in this master |
|---|---|---|---|
| A-51 | Blocker | Core product boundary still depended on the legacy ELIOT Memory OS / Deep Research module taxonomy | S1 now defines standalone core, generic clients, optional ELIOT adapter, and separate research systems |
| A-52 | Blocker | No explicit sole-owner/cutover contract existed for mutable source identity and revisions | S7.2.1 and INV-26/INV-30 define one source owner and a fenced cutover receipt |
| A-53 | Blocker | CAS used a global content-digest layout and omitted access/confidentiality/key/lifecycle domains | S6.3, S7, H3, and H6 define the complete residency key and prohibit cross-domain co-residency/key reuse |
| A-54 | Blocker | Unsaved-buffer rules did not close every durable side channel | INV-28 and S18 prohibit persistence to stores, backups, telemetry, caches, evaluation, and learning/training inputs |
| A-55 | Major | Core read grant embedded the client-specific `WorkScope` name | S19 uses generic `client_scope_ref` and `scope_domain_id`; S32.3 performs the ELIOT mapping |
| A-56 | Major | S32 incorrectly claimed the current ELIOT books still required a terminology migration | stale claim removed; S32.3 now matches the final external-provider boundary |
| A-57 | Major | ELIOT and Eliot Research adapters were not isolated strongly enough from the core | C30/S31/S32/H16/P14 make adapters optional leaf packages with no reverse authority or core dependency |
| A-58 | Blocker | Search and Research claimed the same `eliotr.normalized.v1` protocol while disagreeing on `ownership_cutover`, its mandatory receipt, and the canonical `content.md` path | S32.4 now reproduces the exact Research-owned wire manifest and states that the field records, but cannot authorize, a separately completed fenced cutover |
| A-59 | Blocker | Baseline Definition of Done required the optional ELIOT adapter, contradicting standalone build/test and disabled-by-default compatibility profiles | H19 requires only the generic client edge; both optional adapters are explicitly `not_required` for baseline acceptance |
| A-60 | Blocker | Source-owner cutover, complete residency identity, and exhaustive unsaved-content non-persistence were specified but not assigned to executable PR exits | P03, P04, and P09 now deliver the state machines and negative fixtures explicitly |
| A-61 | Major | The Eliot Research export profile lacked a delivery slice and exact cross-repository round-trip proof | P14 now owns the optional export adapter, exact schema fixture, unknown-field rejection, ownership-mode validation, and cutover-receipt test |
| A-62 | Blocker | The public boundary named `SearchTaskPlan` and `SearchCandidateSet` without defining either contract, leaving plan authority, view/generation binding, coverage semantics, and vendor-field exclusion underspecified | S19.2 and S23.2 now define both types; P00, H15, P08, and P14 require schema and end-to-end contract proofs |
| A-63 | Blocker | `ownership_cutover_receipt_ref` was mandatory but the receipt body, generation/view bindings, bilateral authorization, and failure semantics were undefined | S7.2.1 now embeds exact `source.owner-cutover.v1`; P00, P03, and P14 require hash, state-machine, mismatch, and partial-authorization tests |

## Final status

```text
Architecture boundary:       coherent 8.4 candidate for P00
Storage topology:            Qdrant-only retrieval
Control journal:             bounded non-search redb
Membership isolation:        specified
Publication linearization:   specified
Source snapshot coherence:   specified
Codex handoff:               embedded 2.7, ready for P00
Runtime:                      absent
Performance evidence:        absent
Product acceptance:          not accepted
```
---

# Part IV — Mechanical Validation and Release Notes

## V1. Single-file contract

```text
Master file: ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md
Embedded Architecture version: 8.4
Embedded Handoff version: 2.7
Architecture section SHA-256: ae4c18ccff256ce4d5fdf91dfd9041236ff6f332b611bae3bd748c2da8ac6a1c
Separate mandatory Search Markdown files: 0
Codex entry point: P00
```

The full-file SHA-256 is intentionally supplied externally with the artifact link because embedding a file's own digest changes the file. The embedded Architecture has a stable independently checkable digest and is the normative input for Codex.

## V2. Mechanical checks required for this master

```text
UTF-8 decode and English active surface: PASS
Markdown fences and heading hierarchy: PASS
Markdown tables: PASS
exact duplicate active headings: 0
embedded Architecture digest extraction: PASS
required Architecture headings S0–S39: PASS
required ownership/residency/view/overlay/client-boundary sections: PASS
exact `eliotr.normalized.v1` Search↔Research schema parity: PASS
baseline acceptance excludes optional adapters: PASS
PR graph covers owner cutover, residency, unsaved-content fence, and export profile: PASS
```

The release artifact MUST regenerate and verify these checks; inherited line/byte counts are not normative.

## V3. Honest execution status

```text
Document coherence review: performed
Runtime implementation: ABSENT
Qdrant Windows capability/fault proof: NOT_EXECUTED
redb/CAS crash proof: NOT_EXECUTED
Performance evidence: ABSENT
Security execution evidence: ABSENT
generic client edge proof: NOT_EXECUTED
optional ELIOT adapter edge proof: NOT_EXECUTED
optional Eliot Research export edge proof: NOT_EXECUTED
Product acceptance: NOT_ACCEPTED
```

## V4. Implementation boundary

Codex begins with P00 only. It MUST NOT create Qdrant, redb, filesystem, Tree-sitter, model or document-provider implementation before the P00 contract proof is returned and reviewed.


## V5. External-repository alignment receipt

```yaml
alignment_date: 2026-08-28
standalone_core: true
optional_eliot_adapter: true
optional_eliot_research_export_adapter: true
source_namespace_single_owner: enforced
complete_object_residency_key: enforced
unsaved_durable_side_channels: prohibited
normalized_bundle_wire_schema: exact
ownership_cutover_requires_separate_receipt: true
optional_adapters_required_for_baseline: false
embedded_architecture_sha256: ae4c18ccff256ce4d5fdf91dfd9041236ff6f332b611bae3bd748c2da8ac6a1c
```
