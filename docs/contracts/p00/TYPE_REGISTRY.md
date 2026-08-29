# P00 supporting type registry

This registry closes helper types referenced by the field-level schemas. A writer cannot replace one
with a local `String`, `Vec`, map or vendor object.

## Visibility classes

| Class | Meaning |
|---|---|
| `ProviderWire` | may appear in `ProviderEnvelope`; must pass disclosure review |
| `SharedDomain` | shared cross-package contract; not automatically provider-visible |
| `ServerRecord` | persisted or process-local server state; forbidden in provider result variants |
| `PackageOpaque` | capability-owned type exposed only as an opaque public handle/reference |

Visibility is explicit per record. Implementing `Serialize` for internal storage does not make a
`ServerRecord` legal in provider JSON.

## Bounded collection and byte wrappers

```yaml
LimitClass:
  name: ProfileId
  max_items: u32 | null
  max_bytes: u64 | null
  max_depth: u16 | null
```

The accepted contract publishes one immutable `ContractBoundsV1` table mapping every named limit class
to exact maxima. A zero maximum means the field is disabled, never unlimited.

```text
BoundedList<T, LimitClass>      ordered, duplicates allowed only when the field says so
BoundedSet<T, LimitClass>       canonical uniqueness and deterministic iteration order
BoundedMap<K,V,LimitClass>      canonical unique keys
BoundedText<LimitClass>         validated UTF-8
BoundedBytes<LimitClass>        arbitrary bytes
BoundedCanonicalBytes<LimitClass> bytes already validated against a named canonical codec
BoundedOpaqueBytes<LimitClass>  bytes with no semantic parsing by the consumer
BoundedTextOrBytes              tagged text | bytes; encoding is never guessed
```

All decoders enforce limits before full allocation where framing permits it. Recursive records such as
`NativeAnchor.archive_member` use a named depth limit.

## Opaque and display wrappers

| Type | Visibility | Rule |
|---|---|---|
| `OpaqueId` | SharedDomain | non-empty bounded identity with no consumer-side parsing |
| `OpaqueRef` | SharedDomain | non-empty bounded reference; possession grants no authority |
| `OpaqueCanonicalBytes` | SharedDomain | bounded bytes validated by the producing contract |
| `OpaqueHandleToken` | ProviderWire | CSPRNG bearer locator; redacted; current auth still required |
| `BoundedDisplayName` | ProviderWire when authorized | display-only UTF-8; never identity |
| `BoundedDisplayPath` | ProviderWire only after disclosure check | original display path; never authorization or identity |
| `BoundedName` | SharedDomain | normalized subject-name input/output |
| `BoundedSymbolKey` | SharedDomain | normalized qualified/exact symbol key |
| `BoundedExpression` | SharedDomain | descriptive configuration predicate; not executable code |
| `BoundedObservation` | ProviderWire | content-minimized descriptive observation |
| `BoundedBehaviorSignature` | SharedDomain | deterministic descriptive comparison signature |
| `BoundedNonContentMetadata` | ProviderWire | allowlisted scalar/count/profile metadata only |
| `BoundedNonContentRankingTrace` | ProviderWire | no query/source text, path, vendor payload or inaccessible facet |
| `OpaqueAuthorizedFacetValue` | ProviderWire | display value resolved only after authorization |

## Identity and reference registry

Every entry is a distinct newtype/tagged union.

```text
InstallationId, InstallationIncarnationId, DataRootId, BindingId,
WorkspaceId, RootBindingId, PathBindingId, RepositoryLineageId,
CorpusId, ReferencePortfolioId, SourceNamespaceId, SourceId,
SourceMembershipId, ProjectionMembershipId, SourceRevisionId,
MaterializationId, RepresentationId, UnitId, AccessPartitionId,
ScoringPartitionId, ScoringDocumentId, AccessPolicyBindingId,
ResidencyPolicyBindingId, ScopeDomainId, AccessDomainId,
ConfidentialityDomainId, EncryptionKeyDomainId, RetentionDomainId,
ErasureDomainId, GrantId, RequestId, PlanId, CandidateId, CutoverId,
BufferSnapshotId, ImportedSnapshotId, HandleId, ContinuationId,
PublicationIntentId, PublicationReceiptId.
```

Counter/version wrappers:

```text
OwnerEpoch, Epoch, NonZeroRevision, PortfolioRevision,
CollectionRouteRevision, CatalogRevision, MembershipRevision,
AccessPolicyRevision, ShadowFenceRevision, PurgeFenceRevision,
ObservationCursorRevision, PolicyRevision.
```

Digest/profile wrappers:

```text
Blake3Digest32, Sha256Digest32, VersionedContentDigest,
SourceOwnerGeneration, ObjectResidencyKeyDigest, PlanFingerprint,
ArtifactDigest, DigestRef, HandleTokenDigest, ProfileId, RuleId,
RecipeFamilyId, ReceiptRef, GitObjectId.
```

Tagged reference unions:

```yaml
CorpusOrPortfolioId:
  corpus: CorpusId
  portfolio: ReferencePortfolioId

WorkspaceOrCorpusRef:
  workspace: WorkspaceId
  corpus: CorpusId

SourceViewRef:
  source_view_digest: Blake3Digest32
  workspace_view_revision_ref: WorkspaceViewRevisionId | null

AuthorizedScopeRef:
  scope_domain_id: ScopeDomainId
  authorized_scope_digest: Blake3Digest32

ExactScanPlanRef:
  plan_id: PlanId
  plan_fingerprint: PlanFingerprint
```

Exactly one tagged-union variant is present.

## Baseline semantic registries

```text
AssuranceClass = exact_bytes | mapped_text | lossy_text | descriptive_only
ObservationFreshness = current_confirmed | observed_with_age | gap_detected | unknown
EvidenceRole = definition | reference | test | documentation | caller | configuration
Modality = code | text | document | image | archive | mixed
```

`EntityKind` is a versioned keyword registry rather than an arbitrary string. Baseline values:

```text
function, method, type, trait, impl, module, field, constant, static,
macro, variable, parameter, file, section, test, document, table,
image_region, unknown
```

A provider-specific entity kind maps to one baseline value plus an optional private profile-qualified
subkind; the subkind cannot affect access or exact identity unless a future contract versions it.

## Protocol support records

```yaml
ProtocolRange:
  minimum: ProtocolVersion
  maximum: ProtocolVersion

OptionalProviderState:
  profile_id: ProfileId
  state: absent | stopped | starting | ready | degraded | quarantined
  degraded_reason_codes: bounded_set<SearchReasonCodeV1>
  artifact_identity_digest: Blake3Digest32 | null

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

BoundedProgressCounts:
  completed_legs: u32
  total_planned_legs: u32
  nominated_candidates: u32
  validated_candidates: u32
  omitted_or_failed_legs: u32
```

Progress counts reveal no inaccessible population totals.

## Coverage support records

```yaml
LegDescriptor:
  leg_ref: OpaqueId
  leg_kind: direct | exact | structural | lexical | semantic | rerank
  scoring_partition_ref: OpaqueRef | null
  profile_id: ProfileId

LegExecutionSummary:
  leg_ref: OpaqueId
  state: completed | partial | cancelled | failed | discarded_contaminated
  nominated_count: u32
  validated_count: u32
  reason_codes: bounded_set<SearchReasonCodeV1>

CoverageGap:
  gap_ref: OpaqueId
  kind: unavailable_membership | failed_leg | omitted_budget | observation_gap | source_unreadable | validation_gap | access_revoked | purge | provider_degraded
  affected_scope_refs: bounded_list<OpaqueRef>
  reason_codes: bounded_set<SearchReasonCodeV1>
  retryability: never | same_request | after_refresh | after_reconcile

CoverageUnknown:
  unknown_ref: OpaqueId
  description_template_id: OpaqueId
  bounded_metadata: BoundedNonContentMetadata

ObservationFreshness:
  state: current_confirmed | observed_with_age | gap_detected | unknown
  observation_cursor_revision: ObservationCursorRevision
  observed_age_ms: u64 | null
```

Counts and affected scopes are already authorization-filtered. Gap records contain no source body,
excerpt, secret, inaccessible name or absolute path.

## Port support records

```yaml
OperationContext:
  request_id: RequestId
  relative_deadline_ms: u64
  cancellation_ref: PackageOpaque
  budget_ref: OpaqueRef

MutationIdentity:
  operation_id: OpaqueId
  idempotency: retry_same_identity | single_attempt | externally_idempotent

PortReceipt:
  operation_id: OpaqueId
  dependency_generation_digest: Blake3Digest32
  outcome: complete | partial | rejected | cancelled | timed_out
  retryability: never | same_identity | new_operation_after_refresh
  bounded_metadata: BoundedNonContentMetadata

BoundedPage<T>:
  items: bounded_list<T>
  continuation_ref: OpaqueRef | null
  complete: bool

BoundedStream<T>:
  stream_ref: PackageOpaque
  item_limit: u32
  byte_limit: u64
  deadline_ms: u64
```

Cancellation and stream refs are process/package capabilities, not provider-wire values. Concrete
executor, channel, file, socket or vendor request types never appear here.

## Ownership rule

If a new helper type is needed, the contract-change request must classify its visibility, canonical
representation, bounds, disclosure, owner and whether it is serialized. Local aliases that duplicate
this registry are forbidden.
