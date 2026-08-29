# P00 subordinate support schemas

These records close named placeholders used by plans, results, exact reports and capability
descriptors. Every list/string/byte field uses a named limit from `ContractBoundsV1`.

## Ranking and execution summaries

```yaml
BoundedNonContentRankingTrace:
  fusion_profile_id: ProfileId
  fused_rank: u32
  exact_or_entity_boost: none | exact_name | qualified_name | entity_kind
  evidence_role_priority: u16
  portfolio_priority: u16
  lineage_diversity_action: retained | collapsed | capped
  deterministic_tie_break_digest: Blake3Digest32
```

Raw vendor scores, query/source text, inaccessible counts/facets and point payload are forbidden.

```yaml
SafeLeg:
  leg_ref: OpaqueId
  leg_kind: direct | exact | structural | lexical | semantic | rerank
  authorized_scope_ref: AuthorizedScopeRef
  access_partition_id: AccessPartitionId | null
  scoring_partition_id: ScoringPartitionId | null
  projection_membership_ids: bounded_set<ProjectionMembershipId>
  profile_id: ProfileId
  budget: QueryExecutionBudget
  eligibility_predicate_digest: Blake3Digest32
  idf_predicate_digest: Blake3Digest32 | null

LegExecutionSummary:
  leg_ref: OpaqueId
  state: completed | partial | cancelled | failed | discarded_contaminated
  nominated_count: u32
  validated_count: u32
  reason_codes: bounded_set<SearchReasonCodeV1>
  receipt_ref: ReceiptRef
```

A lexical/scoring leg requires identical eligibility and IDF semantics. The digests may differ only
when one is absent because the leg has no IDF population.

## Subject and comparison observations

```yaml
ConfigurationObservation:
  predicate: BoundedExpression
  observation: BoundedObservation
  evidence_handles: bounded_list<SearchSourceHandle>
  assurance: AssuranceClass

BehaviorObservation:
  axis: interface | validation | errors | side_effects | tests | callers | documentation
  summary: BoundedObservation
  evidence_handles: bounded_list<SearchSourceHandle>
  configuration_predicate: BoundedExpression | null
  independent_lineage_count: u32
  assurance: AssuranceClass

BehaviorConflict:
  axis: interface | validation | errors | side_effects | tests | callers | documentation
  left: BehaviorObservation
  right: BehaviorObservation
  conflict_summary: BoundedObservation
  unresolved_reason_codes: bounded_set<SearchReasonCodeV1>
```

`independent_lineage_count` is computed after fork/mirror collapse and contains only authorized
lineages.

## Exact proof support

```yaml
ExactCompletenessRequirements:
  require_every_denominator_item: bool
  require_stable_or_retained_revision: bool
  require_current_observation: bool
  include_authenticated_unsaved_buffers: bool
  fail_on_timeout: bool
  fail_on_cancellation: bool
  fail_on_scope_drift: bool

ExactMatch:
  source_revision_ref: SourceRevisionRef
  native_anchor: NativeAnchor
  match_digest: Blake3Digest32
  matched_byte_length: u64
  predicate_profile_id: ProfileId
  assurance: exact_bytes | mapped_text
  source_handle: SearchSourceHandle

ExactItemFailure:
  source_revision_id: SourceRevisionId
  failure_kind: unreadable | revision_unavailable | scope_changed | timeout | cancelled | unsupported_encoding | predicate_error
  reason_codes: bounded_set<SearchReasonCodeV1>
  bounded_metadata: BoundedNonContentMetadata
```

An exact match never cites Qdrant payload text. `no_match_in_complete_scope` is impossible when any
failure collection is non-empty or a required completeness flag is false.

## Readiness and provider support

```yaml
OptionalProviderState:
  profile_id: ProfileId
  state: absent | stopped | starting | ready | degraded | quarantined
  artifact_identity_digest: Blake3Digest32 | null
  degraded_reason_codes: bounded_set<SearchReasonCodeV1>

MembershipReadiness:
  source_membership_id: SourceMembershipId
  direct_ready: bool
  lexical_ready: bool
  code_ready: bool
  semantic_ready: bool
  document_ready: bool
  visible_epoch: Epoch | null
  observation_freshness: ObservationFreshness
  degraded_reason_codes: bounded_set<SearchReasonCodeV1>
```

A `false` optional readiness bit is planning information, not denial and not authority.

## Progress and non-content metadata

```yaml
BoundedProgressCounts:
  completed_legs: u32
  total_planned_legs: u32
  nominated_candidates: u32
  validated_candidates: u32
  omitted_or_failed_legs: u32

BoundedNonContentMetadata:
  entries: bounded_map<MetadataKey, MetadataScalar>

MetadataScalar:
  boolean: bool
  unsigned: u64
  signed: i64
  duration_ms: u64
  digest: Blake3Digest32
  profile_id: ProfileId
  template_id: OpaqueId
```

Arbitrary strings are not permitted in default non-content metadata. Human text is selected from a
versioned template ID outside security-sensitive serialization.

## Handle expansion support

```yaml
HandleExpansionRequest:
  expansion: excerpt | source_metadata | provenance | continuation
  max_bytes: u64
  requested_anchor: NativeAnchor | null

HandlePermit:
  handle_id: HandleId | ContinuationId
  binding_id: BindingId
  authorization_generation_digest: Blake3Digest32
  disclosure_ceiling: local_only | named_client | exportable
  maximum_bytes: u64
  expires_at: UtcTimestamp
  permit_digest: Blake3Digest32
```

`HandlePermit` is process-local/server-side and cannot be used as a reusable provider authorization
decision. It is revalidated immediately before readback and emission.

## Port receipt support ownership

`OperationContext`, `MutationIdentity`, `PortReceipt`, `BoundedPage` and `BoundedStream` are specified
in `TYPE_REGISTRY.md` for shape consistency but are owned and implemented by `search-ports`, not by
`search-contracts`. Their cancellation/stream capabilities remain package-opaque and non-serializable.
