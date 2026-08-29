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

## Public handle tokens

S26.1 requires default result and continuation handles to be opaque random identifiers. Therefore
provider JSON carries only bearer locators and non-sensitive lifecycle metadata, never the detailed
source/plan record that the daemon uses internally.

```yaml
SearchSourceHandle:
  handle_id: HandleId
  handle_revision: NonZeroRevision
  handle_class: ephemeral | durable_source
  expires_at: UtcTimestamp | null
  opaque_token: OpaqueHandleToken

ContinuationHandle:
  continuation_id: ContinuationId
  expires_at: UtcTimestamp
  opaque_token: OpaqueHandleToken
```

The token has at least 256 bits of CSPRNG entropy, is never deterministically derived from source or
plan data, and is redacted from every default diagnostic surface. `HandleId`/`ContinuationId` alone is
not sufficient to resolve or authorize anything. Token possession grants no access; every use requires
current binding/grant/security/owner/view/residency/purge validation.

## Server-side source-handle records

The grouped fields from S26.2 are represented in server-owned records, not exposed as the wire token.
The record is a closed tagged union because an ephemeral handle may target authenticated unsaved bytes,
while a durable handle may not.

```yaml
SearchSourceHandleRecord:
  ephemeral:
    handle_id: HandleId
    handle_revision: NonZeroRevision
    token_digest: HandleTokenDigest
    binding_id: BindingId
    grant_id: GrantId
    target:
      retained_source:
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
      unsaved_buffer:
        workspace_id: WorkspaceId
        workspace_view_revision_ref: WorkspaceViewRevisionId
        buffer_snapshot_id: BufferSnapshotId
        buffer_version: u64
        native_anchor: NativeAnchor
        excerpt_digest: Blake3Digest32
    created_at: UtcTimestamp
    expires_at: UtcTimestamp
    invalidation_refs: bounded_list<OpaqueRef>
    status: ACTIVE | REVOKED | EXPIRED
  durable_source:
    handle_id: HandleId
    handle_revision: NonZeroRevision
    token_digest: HandleTokenDigest
    binding_id: BindingId
    grant_id: GrantId
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
    retention_lease_ref: OpaqueRef
    created_at: UtcTimestamp
    retention_expiry: UtcTimestamp | null
    invalidation_refs: bounded_list<OpaqueRef>
    status: ACTIVE | REVOKED | EXPIRED
```

Ephemeral records are memory-only and restart-invalid. An unsaved-buffer target never enters redb, CAS,
backup, telemetry, evaluation or a durable record. A durable record requires an immutable retained
revision plus a current retention lease. The public token never embeds or serializes this record.

## Server-side continuation records

```yaml
ContinuationRecord:
  ephemeral_window:
    continuation_id: ContinuationId
    token_digest: HandleTokenDigest
    binding_id: BindingId
    plan_fingerprint: PlanFingerprint
    result_fence: ResultFence
    candidate_window_ref: OpaqueRef
    issued_candidate_identity_set_ref: OpaqueRef
    epoch_pin_ref: OpaqueRef
    created_at: UtcTimestamp
    expires_at: UtcTimestamp
    status: ACTIVE | REVOKED | EXPIRED
  durable_replan_checkpoint:
    continuation_id: ContinuationId
    token_digest: HandleTokenDigest
    binding_id: BindingId
    plan_fingerprint: PlanFingerprint
    result_fence: ResultFence
    durable_job_ref: OpaqueRef
    replan_checkpoint_ref: OpaqueRef
    issued_candidate_identity_set_ref: OpaqueRef
    created_at: UtcTimestamp
    expires_at: UtcTimestamp
    status: ACTIVE | REVOKED | EXPIRED
```

The ephemeral variant owns a bounded candidate window and pin; it is memory-only and restart-invalid.
The durable variant is allowed only for an explicit durable job and stores no process-local pin or
unsaved bytes. Raw Qdrant offsets, cursors, scores and point IDs never appear in the public token.

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
