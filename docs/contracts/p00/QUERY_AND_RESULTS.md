# Grant, snapshot, plan, candidate and exact-proof schemas

Recipe-specific outputs are defined in `RECIPE_RESULTS.md`.

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

Grant lists are ceilings intersected with server-authoritative state. They contain no vendor filters,
collection names, point IDs or unrestricted paths.

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

Every limit is server-clamped. Zero means disabled/none as defined by the field; it never means
unlimited.

## Query snapshot fence

This is the explicit immutable planning snapshot from Architecture S14.1. No generic dependency digest
may replace one of these fields.

```yaml
QuerySnapshotFence:
  installation_incarnation_id: InstallationIncarnationId
  collection_generation_id: CollectionGenerationId | null
  visible_epoch: Epoch | null
  collection_route_revision: CollectionRouteRevision
  catalog_revision: CatalogRevision
  membership_revision: MembershipRevision
  reference_portfolio_revision: PortfolioRevision | null
  access_policy_revision: AccessPolicyRevision
  shadow_fence_revision: ShadowFenceRevision
  purge_fence_revision: PurgeFenceRevision
  overlay_revision: OverlayRevision
  observation_cursor_revision: ObservationCursorRevision
  observation_freshness: ObservationFreshness
  source_view: SourceView
  workspace_view_revision_ref: WorkspaceViewRevisionId | null
  lexical_profile_ids: bounded_list<ProfileId>
  snapshot_fingerprint: QuerySnapshotFingerprint
```

`collection_generation_id` and `visible_epoch` are both absent only for a direct-only plan. A lexical or
indexed leg requires both. `snapshot_fingerprint` is computed from every preceding field using
`eliot-search/query-snapshot-fingerprint/v1` deterministic CBOR.

An unresolved observation gap cannot produce a strict `current_workspace` plan. A relaxed plan may
retain `gap_detected` only when the recipe/exactness requirements allow truthful incomplete coverage.

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
  query_snapshot_fence: QuerySnapshotFence
  source_owner_fences: bounded_list<SourceOwnerFence>
  selected_membership_ids: bounded_list<SourceMembershipId>
  profile_fence:
    fusion_profile_id: FusionProfileId
    projection_profile_set_ids: bounded_list<ProjectionProfileSetId>
    optional_provider_profile_ids: bounded_list<ProfileId>
  overlay_snapshot_refs: bounded_list<OpaqueRef>
  query_execution_budget: QueryExecutionBudget
  exactness_requirements: ExactnessRequirements
  additional_state_dependencies: bounded_list<StateDependency>
  plan_fingerprint: PlanFingerprint
  created_at: UtcTimestamp
  expires_at: UtcTimestamp
```

```yaml
SourceOwnerFence:
  source_namespace_id: SourceNamespaceId
  source_owner_generation: SourceOwnerGeneration

StateDependency:
  kind: materializer_profile | unitizer_profile | enricher_profile | provider_capability | overlap_route_proof | retention_lease
  identity_digest: Blake3Digest32

ExactnessRequirements:
  required_denominator: candidate_scope | complete_scope | unknown_allowed
  require_current_observation: bool
  allow_truthful_partial: bool
```

`additional_state_dependencies` may add package/profile dependencies but cannot hide or replace catalog,
membership, portfolio, access, shadow, purge, overlay, observation, source-view, route, epoch or lexical
profile fields.

`PlanFingerprint` hashes the normalized recipe request, grant/client scope fences,
`QuerySnapshotFence`, source-owner fences, selected memberships, profile fence, overlay refs, budget,
exactness requirements and additional dependencies. `plan_id`, timestamps and the fingerprint itself are
excluded unless the accepted canonical fixture explicitly treats `plan_id` as a deterministic digest
projection.

## Emission fence

Live security and owner state may become more restrictive after planning. A result preserves the
planning snapshot and separately records the latest state that authorized emission.

```yaml
EmissionSecurityFence:
  access_policy_revision: AccessPolicyRevision
  live_deny_generation: u64
  shadow_fence_revision: ShadowFenceRevision
  purge_fence_revision: PurgeFenceRevision
  checked_at: UtcTimestamp
  receipt_ref: ReceiptRef

ResultFence:
  planned_snapshot: QuerySnapshotFence
  emission_source_owner_fences: bounded_list<SourceOwnerFence>
  emission_security_fence: EmissionSecurityFence
  result_fingerprint: Blake3Digest32
```

If a load-bearing route/view/profile/catalog dependency drifts, execution replans or returns explicit
stale/incomplete coverage; it does not rewrite `planned_snapshot`. If a restrictive security or owner
state changes, affected legs/candidates are revalidated under the latest emission fence.

## Candidate result

Architecture S23 removes stale, unreadable, inaccessible and otherwise invalid candidates before
projection. Therefore `candidates` contains only validated, source-backed candidates.

```yaml
SearchCandidateSet:
  request_id: RequestId
  plan_id: PlanId
  plan_fingerprint: PlanFingerprint
  result_fence: ResultFence
  candidates: bounded_list<ValidatedSearchCandidate>
  coverage: Coverage
  continuation_handle: ContinuationHandle | null
  result_validation_receipt_ref: ReceiptRef

ValidatedSearchCandidate:
  candidate_id: CandidateId
  source_handle: SearchSourceHandle
  evidence_role: EvidenceRole
  entity_kind: EntityKind | null
  assurance: exact_bytes | mapped_text | lossy_text | descriptive_only
  freshness: current_confirmed | observed_with_age | gap_detected | unknown
  ranking_trace: BoundedNonContentRankingTrace
  reason_codes: bounded_set<SearchReasonCodeV1>
  candidate_validation_receipt_ref: ReceiptRef
```

`ValidatedSearchCandidate.reason_codes` cannot contain `STALE`, `UNREADABLE`, `ACCESS_REVOKED` or
`PURGED`. Those outcomes are validation gaps, not evidence candidates.

```yaml
Coverage:
  requested_legs: bounded_list<LegDescriptor>
  executed_legs: bounded_list<LegExecutionSummary>
  represented_memberships: bounded_set<SourceMembershipId>
  represented_source_lineages: bounded_set<RepositoryLineageId>
  omitted_or_failed_legs: bounded_list<CoverageGap>
  candidate_validation_gaps: bounded_list<CandidateValidationGap>
  observation_freshness: ObservationFreshness
  unknowns: bounded_list<CoverageUnknown>
  denominator_kind: candidate_scope | complete_scope | unknown

CandidateValidationGap:
  nominated_candidate_ref: OpaqueId
  source_revision_ref: SourceRevisionRef | null
  reason: STALE | UNREADABLE | ACCESS_REVOKED | PURGED | SOURCE_REVISION_UNAVAILABLE
  affected_leg_refs: bounded_list<OpaqueId>
  contaminated_rank_leg: bool
  disposition: dropped | replan_requested | gap_reported
```

A gap carries no excerpt or evidence-bearing source handle. `source_revision_ref` is absent whenever the
latest authorization/disclosure state does not permit revealing it. If a revoked population influenced
scoring or IDF, the entire leg is contaminated and discarded/replanned. `complete_scope` is valid only
from an accepted exact execution report.

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

Text/Git ranges require `start <= end` and bounds within the exact source length. Buffer positions are
lexicographically ordered in the declared encoding. PDF coordinates are finite (no NaN/infinity),
`page_1 >= 1`, and `x0 <= x1`, `y0 <= y1`. Archive nesting is bounded by `ContractBoundsV1`. Lossy
mappings cannot claim raw-byte exactness.

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

`no_match_in_complete_scope` requires every denominator item and proves only the stated exact
predicate, never absence of a semantic analogue.
