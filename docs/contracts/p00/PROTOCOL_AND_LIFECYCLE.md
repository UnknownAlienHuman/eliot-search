# Provider protocol, handles, security and lifecycle schemas

## Protocol version and envelope

```yaml
ProtocolVersion:
  major: u16
  minor: u16

ProviderEnvelope:
  protocol_major: u16
  protocol_minor: u16
  installation_incarnation_id: InstallationIncarnationId
  binding_id: BindingId
  connection_sequence: u64
  request_id: RequestId
  message_kind: hello | request | progress | result | error | cancel | cancelled
  relative_deadline_ms: u64 | null
  body: ProviderBodyV1
```

`body` is a tagged union matching `message_kind`:

```yaml
ProviderBodyV1:
  hello:
    peer_role: daemon | standalone_cli | client_adapter | worker
    pairing_proof_ref: OpaqueRef
    supported_protocol_range: ProtocolRange
    requested_capability_digest: Blake3Digest32 | null
  request:
    grant: SearchReadGrantClaims
    recipe_request: SearchRecipeRequest
  progress:
    event_sequence: u64
    phase: accepted | planning | retrieving | validating | projecting
    bounded_counts: BoundedProgressCounts
    degraded_reason_codes: bounded_set<SearchReasonCodeV1>
  result:
    event_sequence: u64
    result: RecipeResultV1
  error:
    code: ProtocolErrorCode | SearchReasonCodeV1
    retryability: never | same_request | new_request_after_refresh
    message_template_id: OpaqueId
    bounded_metadata: BoundedNonContentMetadata
  cancel:
    target_request_id: RequestId
  cancelled:
    target_request_id: RequestId
    terminal: bool
```

Baseline limits: 8 MiB frame, 32 in-flight requests per connection, monotonic sequence, no compression
and no fragmented-message assembly.

## Capability descriptor

```yaml
SearchProviderCapabilityDescriptor:
  provider_protocol_version: ProtocolVersion
  installation_id: InstallationId
  installation_incarnation_id: InstallationIncarnationId
  data_root_identity: OpaqueId
  owner_epoch: OwnerEpoch
  source_owner_generations: bounded_list<SourceOwnerFence>
  supported_recipes: bounded_set<RecipeIdV1>
  available_profiles: bounded_set<ProfileId>
  optional_provider_states: bounded_list<OptionalProviderState>
  visible_epoch: Epoch | null
  collection_route_revision: CollectionRouteRevision
  access_policy_generation: AccessPolicyRevision
  source_inventory_revision: CatalogRevision
  observation_freshness: ObservationFreshness
  readiness_by_membership: bounded_list<MembershipReadiness>
  degraded_reason_codes: bounded_set<SearchReasonCodeV1>
```

Only memberships visible to the binding are represented. Capability availability grants no authority.

## Source handles

```yaml
SearchSourceHandle:
  handle_id: HandleId
  handle_revision: NonZeroRevision
  durability: ephemeral | durable_source
  binding_id: BindingId
  source_namespace_id: SourceNamespaceId
  source_owner_generation: SourceOwnerGeneration
  source_revision_ref: SourceRevisionRef
  source_view: SourceView
  workspace_view_revision_ref: WorkspaceViewRevisionId | null
  native_anchor: NativeAnchor
  excerpt_digest: Blake3Digest32
  materialization_profile_id: ProfileId
  assurance_ceiling: exact_bytes | mapped_text | lossy_text | descriptive_only
  object_residency_key_digest: ObjectResidencyKeyDigest
  retention_expiry: UtcTimestamp | null
  invalidation_refs: bounded_list<OpaqueRef>
```

A durable handle requires an immutable retained revision and cannot target unsaved bytes. Every
expansion rechecks binding, grant, owner generation, view, residency and purge state. Possession grants
no access.

```yaml
ContinuationHandle:
  continuation_id: ContinuationId
  binding_id: BindingId
  durability: ephemeral_in_memory | durable_replan_checkpoint
  plan_fingerprint: PlanFingerprint
  expires_at: UtcTimestamp
  opaque_token: BoundedOpaqueBytes
```

The token contains no raw Qdrant cursor, score, path or source bytes.

## Security mutation state

```yaml
SecurityMutationBarrierState:
  security_domain_ref: OpaqueRef
  phase: ACQUIRED | DURABLE_COMMITTED | LIVE_SNAPSHOT_PUBLISHED | DEPENDENTS_INVALIDATED | ACKNOWLEDGED | FAIL_CLOSED
  access_policy_revision: AccessPolicyRevision
  live_deny_generation: u64
  mutation_receipt_ref: ReceiptRef

LiveDenySnapshotRef:
  security_domain_ref: OpaqueRef
  live_deny_generation: u64
  snapshot_digest: Blake3Digest32
```

Acknowledgement occurs only after durable state, live snapshot and dependent invalidation are
observable.

## Publication support records

State spellings preserve the S13 state machine.

```yaml
PublicationIntent:
  publication_intent_id: PublicationIntentId
  target_epoch: Epoch
  prepared_manifest_ref: ReceiptRef
  owner_source_membership_access_guards: bounded_list<StateDependency>
  state: PREPARED | INTENT_DURABLE | NEW_POINTS_ACKNOWLEDGED | OLD_POINTS_CLOSED_ACKNOWLEDGED | READBACK_VERIFIED | CONTROL_COMMITTED | RECLAIMABLE | COMPENSATING | ABORTED | INVALIDATION_ONLY_COMMITTED | PUBLICATION_BLOCKED

PublicationReceipt:
  publication_receipt_id: PublicationReceiptId
  target_epoch: Epoch
  exact_new_manifest_ref: ReceiptRef
  exact_retired_manifest_ref: ReceiptRef
  readback_digest: Blake3Digest32
  control_commit_revision: NonZeroRevision

AbandonedPublicationFence:
  publication_intent_id: PublicationIntentId
  collection_generation_id: CollectionGenerationId
  excluded_projection_memberships: bounded_set<ProjectionMembershipId>
  excluded_partition_refs: bounded_set<OpaqueRef>
  fence_revision: NonZeroRevision
  receipt_ref: ReceiptRef
```

Uncommitted intents never change visible epoch. Skipped epochs are not reused. Abandonment is legal
only after the exclusion fence is active before retrieval and IDF.

## Purge and restore

```yaml
PurgeReceipt:
  request_ref: OpaqueRef
  fence_revision: PurgeFenceRevision
  logical_non_accessibility: pending | complete | failed
  index_deletion: not_applicable | pending | complete | partial | failed
  cache_deletion: not_applicable | pending | complete | partial | failed
  backup_snapshot_status: not_present | pending | retained_tombstone | unresolved
  physical_secure_erase:
    status: not_guaranteed | evidence_available
    evidence_ref: ReceiptRef | null
  revoked_handle_count: u64
  tombstone_ref: ReceiptRef

PairedRecoveryManifest:
  installation_incarnation_id: InstallationIncarnationId
  redb_checkpoint_digest: Blake3Digest32
  qdrant_snapshot_identity: OpaqueId
  collection_generation_id: CollectionGenerationId
  schema_identity_digest: Blake3Digest32
  committed_visible_epoch: Epoch
  latest_publication_receipt_ref: ReceiptRef
  purge_tombstone_generation: u64

RestoreDecision:
  state: RESTORE_PENDING_REVALIDATION | DIRECT_ONLY | INDEXED_ADMITTED | QUARANTINED
  reason_codes: bounded_set<SearchReasonCodeV1>
  validation_receipt_refs: bounded_list<ReceiptRef>
```

`evidence_ref` must be absent when secure erase is not guaranteed. Ordinary index reclamation cannot
satisfy a security purge receipt.
