# Source, ownership, residency and view schemas

All lists are bounded. All `opaque` values are strong bounded wrappers, not `String` aliases.

## Installation and route

```yaml
SearchInstallation:
  installation_id: InstallationId
  installation_incarnation_id: InstallationIncarnationId
  data_root_id: DataRootId
  owner_epoch: OwnerEpoch
  active_mode: standalone | managed_client

CollectionRoute:
  collection_generation_id: CollectionGenerationId
  physical_collection_name: OpaqueRef
  schema_identity_digest: Blake3Digest32
  qualified_qdrant_build: ArtifactDigest
  committed_visible_epoch: Epoch
  collection_route_revision: CollectionRouteRevision
```

A copied/restored data root gets a new incarnation unless an exact paired recovery manifest proves
continuity.

## Repository and workspace

```yaml
RepositoryLineage:
  lineage_id: RepositoryLineageId
  vcs_kind: git | none
  canonical_remote_fingerprints: bounded_set<DigestRef>
  fork_relations: bounded_set<RepositoryLineageId>

WorkspaceInstance:
  workspace_id: WorkspaceId
  lineage_id: RepositoryLineageId
  root_binding_id: RootBindingId
  worktree_or_checkout_identity: OpaqueId
```

Nested repositories and submodules remain distinct workspaces.

## Namespace ownership

```yaml
SourceNamespaceOwnership:
  source_namespace_id: SourceNamespaceId
  owner_system_id: OpaqueId
  owner_installation_incarnation_id: InstallationIncarnationId
  owner_epoch: OwnerEpoch
  ownership_record_revision: NonZeroRevision
  source_owner_generation: SourceOwnerGeneration # BLAKE3-256
  source_admission_policy_revision: PolicyRevision
  status: active | cutover_prepared | fenced | retired
  cutover_receipt_ref: ReceiptRef | null
```

`source_owner_generation` is the domain-separated digest of namespace, owner system, installation
incarnation, owner epoch, ownership-record revision and status.

```yaml
SourceOwnerCutoverReceipt:
  protocol: source.owner-cutover.v1
  cutover:
    cutover_id: CutoverId
    source_namespace_id: SourceNamespaceId
    identity_mapping_digest: Blake3Digest32
    prepared_at: UtcTimestamp
    effective_at: UtcTimestamp
  old_owner:
    owner_system_id: OpaqueId
    source_owner_generation_before_fence: SourceOwnerGeneration
    fence_revision: NonZeroRevision
    final_source_view_ref: SourceViewRef
    final_revision_set_digest: Blake3Digest32
    terminal_status: fenced | retired
  new_owner:
    owner_system_id: OpaqueId
    source_owner_generation_after_activation: SourceOwnerGeneration
    activation_revision: NonZeroRevision
    admitted_revision_set_digest: Blake3Digest32
    status: active
  validation:
    compatibility_receipt_refs: bounded_list<ReceiptRef>
    integrity_receipt_refs: bounded_list<ReceiptRef>
    unresolved_sources_and_reasons: bounded_list<UnresolvedSource>
  authorization:
    old_owner_authorization_ref: OpaqueRef
    new_owner_authorization_ref: OpaqueRef
    issued_at: UtcTimestamp
```

Canonical manifest-body SHA-256 remains
`b659806e37a4bc60ea67b4416e35212f559213bbadb28618b7edcee686b9277e`.

## Source identity and revision occurrence

```yaml
SourceIdentity:
  source_namespace_id: SourceNamespaceId
  source_id: SourceId
  identity_kind: ntfs_file | git_blob_lineage | imported_object | admitted_virtual_snapshot
  stable_identity_components: OpaqueCanonicalBytes

PathBinding:
  binding_id: PathBindingId
  source_id: SourceId
  workspace_id: WorkspaceId
  display_path: BoundedDisplayPath
  canonical_path_key: OpaqueCanonicalBytes
  first_seen_revision: SourceRevisionId
  last_seen_revision: SourceRevisionId | null

SourceRevision:
  revision_id: SourceRevisionId
  source_id: SourceId
  occurrence_sequence: NonZeroU64
  content_digest: Blake3Digest32
  byte_length: u64
  observed_at: UtcTimestamp
  acquisition_kind: filesystem | git_object | imported | admitted_ide_snapshot
  stability_receipt_ref: ReceiptRef
  object_residency_key_digest: ObjectResidencyKeyDigest

SourceRevisionRef:
  source_namespace_id: SourceNamespaceId
  source_id: SourceId
  revision_id: SourceRevisionId
  content_digest: Blake3Digest32
  byte_length: u64
```

A revert `A → B → A` creates three revision occurrences. Paths are locators, not identity.

## Membership and portfolio

```yaml
SourceMembership:
  source_membership_id: SourceMembershipId
  corpus_id: CorpusId
  source_id: SourceId
  workspace_id: WorkspaceId
  role: source | test | documentation | generated | vendor | reference
  preparation_profile_id: ProfileId
  access_policy_binding_id: AccessPolicyBindingId
  retention_policy_id: ProfileId
  residency_policy_binding_id: ResidencyPolicyBindingId
  membership_revision: MembershipRevision

ReferencePortfolioRevision:
  portfolio_id: ReferencePortfolioId
  portfolio_revision: PortfolioRevision
  display_name: BoundedDisplayName
  included_scopes: bounded_list<WorkspaceOrCorpusRef>
  membership_precedence: bounded_list<SourceMembershipId>
  lineage_collapse_policy_id: ProfileId
  role_filters: bounded_set<source | test | documentation | reference>
  access_policy_binding_id: AccessPolicyBindingId
```

One projection membership binds exactly one source membership.

## Residency and preparation

```yaml
SearchObjectResidencyKey:
  scope_domain_id: ScopeDomainId
  access_domain_id: AccessDomainId
  confidentiality_domain_id: ConfidentialityDomainId
  encryption_key_domain_id: EncryptionKeyDomainId
  retention_domain_id: RetentionDomainId
  erasure_domain_id: ErasureDomainId
  versioned_content_digest: VersionedContentDigest

SourceResidencyProfileRef:
  residency_policy_binding_id: ResidencyPolicyBindingId
  policy_revision: PolicyRevision
  profile_id: ProfileId

Materialization:
  materialization_id: MaterializationId
  source_revision_id: SourceRevisionId
  materializer_profile_id: ProfileId
  canonical_object_digest: Blake3Digest32
  object_residency_key_digest: ObjectResidencyKeyDigest
  native_coordinate_map_digest: Blake3Digest32
  loss_map_digest: Blake3Digest32
  assurance_ceiling: exact_bytes | mapped_text | lossy_text | descriptive_only

Representation:
  representation_id: RepresentationId
  materialization_id: MaterializationId
  unitizer_profile_id: ProfileId
  enrichment_profile_ids: bounded_list<ProfileId>
  unit_manifest_digest: Blake3Digest32

ProjectionMembership:
  projection_membership_id: ProjectionMembershipId
  source_membership_id: SourceMembershipId
  representation_id: RepresentationId
  access_partition_id: AccessPartitionId
  scoring_partition_id: ScoringPartitionId
  projection_schema_id: ProfileId

UnitOccurrence:
  unit_id: UnitId
  representation_id: RepresentationId
  unit_kind: file | section | symbol | reference | test | doc | table | image_region
  ordinal: u64
  native_anchor: NativeAnchor
  structural_identity: OpaqueId | null
  configuration_predicate: BoundedExpression | null
```

## Source view

Use a tagged union; nullable fields from the prose schema are not simultaneously legal.

```yaml
SourceView:
  working_tree_current:
    workspace_instance_id: WorkspaceId
    workspace_view_revision_ref: WorkspaceViewRevisionId
  git_index:
    workspace_instance_id: WorkspaceId
    workspace_view_revision_ref: WorkspaceViewRevisionId
  git_commit:
    workspace_instance_id: WorkspaceId
    git_commit_oid: GitObjectId
  imported_snapshot:
    imported_snapshot_id: ImportedSnapshotId
  retained_revision:
    retained_revision_id: SourceRevisionId
```

```yaml
WorkspaceViewRevision:
  workspace_view_revision_id: WorkspaceViewRevisionId
  workspace_instance_id: WorkspaceId
  root_filesystem_identity: OpaqueCanonicalBytes
  repository_lineage_id: RepositoryLineageId | null
  head_commit_and_branch: OpaqueCanonicalBytes | null
  git_index_identity: OpaqueCanonicalBytes | null
  inventory_revision: CatalogRevision
  worktree_observation_cursor: ObservationCursorRevision
  authenticated_ide_overlay_revision: u64
  ignore_and_source_admission_policy_revision: PolicyRevision
```

One compound query uses one source/workspace view revision. Drift forces revalidation or an explicit
stale/incomplete result.

## Source admission policy

```yaml
SourceAdmissionPolicy:
  policy_revision: PolicyRevision
  denied_system_locations: bounded_list<RuleId>
  denied_filename_and_format_classes: bounded_list<RuleId>
  secret_and_private_key_detectors: bounded_list<ProfileId>
  generated_vendor_and_binary_policy_ref: OpaqueRef
  maximum_limits:
    max_source_bytes: u64
    max_archive_bytes: u64
    max_archive_members: u32
    max_materialized_bytes: u64
    max_expansion_ratio: u32
  sensitivity_classes: bounded_set<public | project | confidential | secret_candidate>
  explicit_override_authority_ref: OpaqueRef
  disclosure_and_logging_policy_ref: OpaqueRef
```

Unknown policy fields fail closed. Detection receipts never contain matched secret material.
