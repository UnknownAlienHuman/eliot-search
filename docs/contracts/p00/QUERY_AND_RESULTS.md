# Grant, plan, result and exact-proof schemas

## Read grant

```yaml
SearchReadGrantClaims:
  grant_id: GrantId
  installation_id: InstallationId
  installation_incarnation_id: InstallationIncarnationId
  binding_id: BindingId
  principal_opaque_id: OpaqueId
  client_scope_ref: OpaqueRef
  scope_domain_id: ScopeDomainId
  allowed_membership_ids: bounded_set<SourceMembershipId>
  allowed_corpus_or_portfolio_ids: bounded_set<CorpusOrPortfolioId>
  reference_portfolio_revision: PortfolioRevision | null
  allowed_access_partitions: bounded_set<AccessPartitionId>
  allowed_modalities: bounded_set<Modality>
  permitted_recipe_families: bounded_set<RecipeFamilyId>
  maximum_budget_class: ProfileId
  sensitivity_ceiling: public | project | confidential | secret_candidate
  disclosure_ceiling: local_only | named_client | exportable
  source_read_permission: bool
  exact_scan_permission: bool
  issued_boot_id: OpaqueId
  issued_at: UtcTimestamp
  expires_at: UtcTimestamp
  nonce: OpaqueId
  revocation_generation: u64
```

Grant lists are authorization ceilings, then intersected with server-authoritative state. They do not
contain Qdrant filters, collection names, point IDs or unrestricted paths.

## Execution budget

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

Every limit is server-clamped. Zero has explicit schema meaning (`disabled` or `none`) and is never
interpreted as unlimited.

## Task plan

```yaml
SearchTaskPlan:
  plan_id: PlanId
  provider_protocol_version: ProtocolVersion
  request_id: RequestId
  recipe_request_digest: Blake3Digest32
  grant_fence:
    grant_id: GrantId
    revocation_generation: u64
  client_scope_fence:
    client_scope_ref: OpaqueRef
    scope_domain_id: ScopeDomainId
  source_view: SourceView
  workspace_view_revision_ref: WorkspaceViewRevisionId | null
  source_owner_fences: bounded_list<SourceOwnerFence>
  selected_membership_ids: bounded_list<SourceMembershipId>
  reference_portfolio_revision: PortfolioRevision | null
  security_fence:
    access_policy_revision: AccessPolicyRevision
    live_deny_generation: u64
    shadow_fence_revision: ShadowFenceRevision
    purge_fence_revision: PurgeFenceRevision
  route_fence:
    collection_generation_id: CollectionGenerationId | null
    visible_epoch: Epoch | null
    collection_route_revision: CollectionRouteRevision
  profile_fence:
    lexical_profile_ids: bounded_list<ProfileId>
    fusion_profile_id: ProfileId
    optional_provider_profile_ids: bounded_list<ProfileId>
  overlay_snapshot_refs: bounded_list<OpaqueRef>
  query_execution_budget: QueryExecutionBudget
  exactness_requirements: ExactnessRequirements
  state_dependencies: bounded_list<StateDependency>
  plan_fingerprint: PlanFingerprint
  created_at: UtcTimestamp
  expires_at: UtcTimestamp
```

```yaml
SourceOwnerFence:
  source_namespace_id: SourceNamespaceId
  source_owner_generation: SourceOwnerGeneration

StateDependency:
  kind: catalog | membership | access | deny | shadow | purge | route | profile | observation | overlay
  identity_digest: Blake3Digest32

ExactnessRequirements:
  required_denominator: candidate_scope | complete_scope | unknown_allowed
  require_current_observation: bool
  allow_truthful_partial: bool
```

`PlanFingerprint` is BLAKE3-256 over canonical, domain-separated serialization of every load-bearing
field except the fingerprint itself.

## Candidate result

```yaml
SearchCandidateSet:
  request_id: RequestId
  plan_id: PlanId
  plan_fingerprint: PlanFingerprint
  source_view: SourceView
  workspace_view_revision_ref: WorkspaceViewRevisionId | null
  result_fence:
    source_owner_fences: bounded_list<SourceOwnerFence>
    access_policy_revision: AccessPolicyRevision
    live_deny_generation: u64
    collection_route_revision: CollectionRouteRevision
    visible_epoch: Epoch | null
  candidates: bounded_list<SearchCandidate>
  coverage: Coverage
  continuation_handle: ContinuationHandle | null
  result_validation_receipt_ref: ReceiptRef
```

```yaml
SearchCandidate:
  candidate_id: CandidateId
  source_handle: SearchSourceHandle
  evidence_role: EvidenceRole
  entity_kind: EntityKind | null
  assurance: exact_bytes | mapped_text | lossy_text | descriptive_only
  freshness: current_confirmed | observed_with_age | gap_detected | unknown
  validation_state: validated | stale | unreadable
  ranking_trace: BoundedNonContentRankingTrace
  reason_codes: bounded_set<SearchReasonCodeV1>
```

```yaml
Coverage:
  requested_legs: bounded_list<LegDescriptor>
  executed_legs: bounded_list<LegExecutionSummary>
  represented_memberships: bounded_set<SourceMembershipId>
  represented_source_lineages: bounded_set<RepositoryLineageId>
  omitted_or_failed_legs: bounded_list<CoverageGap>
  observation_freshness: ObservationFreshness
  unknowns: bounded_list<CoverageUnknown>
  denominator_kind: candidate_scope | complete_scope | unknown
```

`complete_scope` is valid only when an accepted exact execution report proves the denominator.

## Native anchors

```yaml
NativeAnchor:
  text_bytes:
    content_digest: Blake3Digest32
    byte_start_0: u64
    byte_end_exclusive_0: u64
  git_blob_bytes:
    repository_lineage_id: RepositoryLineageId
    commit_oid: GitObjectId
    path_bytes: BoundedBytes
    byte_start_0: u64
    byte_end_exclusive_0: u64
  buffer_range:
    buffer_snapshot_id: BufferSnapshotId
    buffer_version: u64
    position_encoding: utf8_bytes | utf16_code_units | utf32_codepoints
    start_line_0: u64
    start_character_0: u64
    end_line_0: u64
    end_character_0: u64
  pdf_region:
    source_revision_id: SourceRevisionId
    page_1: u64
    coordinate_space: crop_box_points_after_rotation
    x0: f64
    y0: f64
    x1: f64
    y1: f64
  archive_member:
    archive_revision_id: SourceRevisionId
    member_path_bytes: BoundedBytes
    nested_anchor: NativeAnchor
```

Ranges require ordered bounds and exact revision/digest identity. Lossy mappings cannot claim raw-byte
exactness.

## Exact plan and report

```yaml
ExactPredicate:
  kind: literal | regex | qualified_symbol | structural_pattern | record_field
  engine_and_version: ProfileId
  serialized_form: BoundedCanonicalBytes
  input_domain: raw_bytes | decoded_text | structural_ir
  worst_case_complexity_class: ProfileId

ExactScanPlan:
  plan_id: PlanId
  predicate: ExactPredicate
  denominator:
    source_revision_ids: bounded_list<SourceRevisionId>
    inventory_revision: CatalogRevision
  inclusion_policy_digest: Blake3Digest32
  unsaved_buffer_snapshot_ids: bounded_list<BufferSnapshotId>
  completeness_requirements: ExactCompletenessRequirements
  plan_fingerprint: PlanFingerprint

ExactScanPlanRef:
  plan_id: PlanId
  plan_fingerprint: PlanFingerprint
```

```yaml
ExactExecutionReport:
  plan_ref: ExactScanPlanRef
  matched_items: bounded_list<ExactMatch>
  scanned_items: u64
  scanned_bytes: u64
  unreadable_items: bounded_list<ExactItemFailure>
  changed_or_unavailable_items: bounded_list<ExactItemFailure>
  timed_out: bool
  cancelled: bool
  scope_drifted: bool
  coverage: candidate_scope | complete_scope | unknown
  conclusion: matches_found | no_match_in_complete_scope | incomplete
  receipt_ref: ReceiptRef
```

`no_match_in_complete_scope` requires every denominator item and cannot prove absence of a semantic
analogue.
