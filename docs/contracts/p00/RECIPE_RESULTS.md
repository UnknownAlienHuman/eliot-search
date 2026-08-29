# Recipe-specific result schemas

Every result is bounded, carries the same request/plan/result fence and exposes truthful coverage. A
recipe result never contains client belief/admission/finish authority or raw vendor state.

## Common header

```yaml
RecipeResultHeader:
  request_id: RequestId
  plan_id: PlanId
  plan_fingerprint: PlanFingerprint
  result_fence: ResultFence
  coverage: Coverage
  reason_codes: bounded_set<SearchReasonCodeV1>
```

## Subject resolution

```yaml
ResolvedSubject:
  canonical_handle: SearchSourceHandle
  match_basis: explicit_handle | editor_position | qualified_name | exact_name | signature | structural | lexical | semantic
  entity_kind: EntityKind
  normalized_name: BoundedName
  signature_observation: BoundedObservation | null
  configuration_predicate: BoundedExpression | null

SubjectAmbiguitySet:
  requested_selector_digest: Blake3Digest32
  candidates: bounded_list<AmbiguousSubjectCandidate>
  reason_code: AMBIGUOUS_SUBJECT

AmbiguousSubjectCandidate:
  source_handle: SearchSourceHandle
  entity_kind: EntityKind
  match_basis: qualified_name | exact_name | signature | structural | lexical | semantic
  disambiguation_summary: BoundedNonContentMetadata
```

No materially ambiguous subject is silently selected.

## Inspection

```yaml
EntityInspectionResult:
  header: RecipeResultHeader
  subject: ResolvedSubject | SubjectAmbiguitySet
  definitions: bounded_list<EvidenceObservation>
  references: bounded_list<EvidenceObservation>
  callers: bounded_list<EvidenceObservation>
  tests: bounded_list<EvidenceObservation>
  documentation: bounded_list<EvidenceObservation>
  configuration_variants: bounded_list<ConfigurationObservation>
  continuation_handle: ContinuationHandle | null

EvidenceObservation:
  role: EvidenceRole
  source_handle: SearchSourceHandle
  observation: BoundedObservation
  assurance: exact_bytes | mapped_text | lossy_text | descriptive_only
  configuration_predicate: BoundedExpression | null
```

## Entity exploration

```yaml
EntityExplorationResult:
  header: RecipeResultHeader
  root_subject: ResolvedSubject | SubjectAmbiguitySet
  nodes: bounded_list<EntityGraphNode>
  edges: bounded_list<EntityGraphEdge>
  truncated_at_depth: u8 | null
  continuation_handle: ContinuationHandle | null

EntityGraphNode:
  node_id: OpaqueId
  source_handle: SearchSourceHandle
  entity_kind: EntityKind
  normalized_name: BoundedName

EntityGraphEdge:
  from_node_id: OpaqueId
  to_node_id: OpaqueId
  relation: definition | reference | caller | test | documentation | configuration
  assurance: exact_bytes | mapped_text | lossy_text | descriptive_only
  evidence_handle: SearchSourceHandle
```

Nodes/edges are descriptive navigation, not a canonical program graph.

## Cross-repository comparison

```yaml
CrossRepositoryBehaviorSet:
  header: RecipeResultHeader
  local_subject:
    resolved_subject: ResolvedSubject
    definition: SearchSourceHandle
    signature: BoundedObservation | null
    callers: bounded_list<SearchSourceHandle>
    tests: bounded_list<SearchSourceHandle>
    documentation: bounded_list<SearchSourceHandle>
  comparable_implementations: bounded_list<ComparableImplementation>
  comparison: BehaviorComparison
  recommended_reading: bounded_list<SearchSourceHandle>

ComparableImplementation:
  lineage_id: RepositoryLineageId
  match_basis: exact_name | normalized_name | signature | structural | lexical | semantic
  configuration_predicate: BoundedExpression | null
  evidence_roles: bounded_set<EvidenceRole>
  behavior_signature: BoundedBehaviorSignature
  exact_handles: bounded_list<SearchSourceHandle>

BehaviorComparison:
  shared_observations: bounded_list<BehaviorObservation>
  variants: bounded_list<BehaviorObservation>
  outliers: bounded_list<BehaviorObservation>
  locally_absent_observations: bounded_list<BehaviorObservation>
  conflicts: bounded_list<BehaviorConflict>
  unknowns: bounded_list<CoverageUnknown>
```

Forks/mirrors collapse by lineage for independent-evidence summaries. Tests/docs are evidence roles,
not automatic truth; Search does not emit a “correct implementation” verdict.

## Corpus profile

```yaml
CorpusProfileResult:
  header: RecipeResultHeader
  scope: AuthorizedScopeRef
  facets: bounded_list<CorpusFacet>
  readiness: bounded_list<MembershipReadiness>

CorpusFacet:
  dimension: role | language_or_format | entity_kind | lineage | readiness
  value: OpaqueAuthorizedFacetValue
  count: u64
  count_assurance: exact_inventory | filtered_index | partial
```

Only authorized values/counts are emitted. Filtered-index counts use the same base eligibility filter;
partial counts are marked and cannot support completeness.

## Corpus delta

```yaml
CorpusDeltaResult:
  header: RecipeResultHeader
  from_view: SourceView
  to_view: SourceView
  changes: bounded_list<CorpusChange>

CorpusChange:
  kind: source_added | source_removed | revision_changed | membership_changed | representation_changed | symbol_changed | readiness_changed
  source_id: SourceId | null
  source_membership_id: SourceMembershipId | null
  before_ref: OpaqueRef | null
  after_ref: OpaqueRef | null
  evidence_handles: bounded_list<SearchSourceHandle>
  assurance: exact_bytes | mapped_text | lossy_text | descriptive_only
```

A delta never silently compares incompatible or drifting views.

## Provenance

```yaml
ProvenanceResult:
  header: RecipeResultHeader
  subject_handle: SearchSourceHandle
  chain: bounded_list<ProvenanceStep>
  unresolved_steps: bounded_list<CoverageGap>

ProvenanceStep:
  sequence: u32
  kind: source_identity | revision_occurrence | materialization | representation | unit | projection | export | ownership_cutover
  input_refs: bounded_list<OpaqueRef>
  output_ref: OpaqueRef
  profile_or_protocol_id: ProfileId | null
  receipt_ref: ReceiptRef
```

Provenance describes recorded lineage; it does not infer missing transformations.

## Handle expansion

```yaml
HandleExpansionResult:
  header: RecipeResultHeader
  handle: SearchSourceHandle | ContinuationHandle
  authorization_receipt_ref: ReceiptRef
  body: HandleExpansionBody

HandleExpansionBody:
  excerpt:
    source_revision_ref: SourceRevisionRef
    native_anchor: NativeAnchor
    content: BoundedTextOrBytes
    content_digest: Blake3Digest32
    assurance: exact_bytes | mapped_text | lossy_text | descriptive_only
  source_metadata:
    source_revision_ref: SourceRevisionRef
    authorized_display_path: BoundedDisplayPath | null
    modality: Modality
    language_or_format: ProfileId
    provenance_ref: OpaqueRef
  provenance:
    result: ProvenanceResult
  continuation:
    candidates: bounded_list<ValidatedSearchCandidate>
    coverage_delta: Coverage
    next_continuation_handle: ContinuationHandle | null
```

Every expansion reauthorizes live grant/owner/view/residency/purge state. The body is size-clamped before
source readback/emission.

## Tagged recipe result

```yaml
RecipeResultV1:
  locate: SearchCandidateSet
  find_text: SearchCandidateSet
  inspect_entity: EntityInspectionResult
  compare_implementations: CrossRepositoryBehaviorSet
  explore_entity: EntityExplorationResult
  corpus_profile: CorpusProfileResult
  corpus_delta: CorpusDeltaResult
  provenance: ProvenanceResult
  compile_exact_scan: ExactScanPlan
  execute_exact_scan: ExactExecutionReport
  expand_handle: HandleExpansionResult
```

Exactly one variant is present and its tag must match the request recipe.
