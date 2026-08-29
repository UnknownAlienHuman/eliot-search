# P00 supporting type registry

This registry closes helper types referenced by field-level schemas. A writer cannot replace one with
a local `String`, `Vec`, map or vendor object.

## Visibility and owner classes

| Class | Owner | Meaning |
|---|---|---|
| `ProviderWire` | `search-contracts` | legal in `ProviderEnvelope` after disclosure review |
| `SharedDomain` | `search-contracts` | shared record, not automatically provider-visible |
| `ServerRecord` | `search-contracts` schema; owning capability controls state | forbidden in provider result variants |
| `PackageOpaque` | owning capability | cross-boundary only through opaque type/reference |
| `PortSupport` | `search-ports` | operation context/receipt/stream support; not provider wire |

Implementing a storage serializer does not make a `ServerRecord` legal in provider JSON.

## Bounds and collections

```yaml
ContractBoundsV1:
  bounds_revision: NonZeroRevision
  classes: bounded_map<ProfileId, LimitClass>
  table_digest: Blake3Digest32

LimitClass:
  max_items: u32 | null
  max_bytes: u64 | null
  max_depth: u16 | null
```

W0 publishes exact values and a digest. Zero means disabled, never unlimited.

```text
BoundedList<T,L>   ordered; duplicate policy is field-specific
BoundedSet<T,L>    canonical uniqueness and deterministic iteration
BoundedMap<K,V,L>  canonical unique keys
BoundedText<L>     validated UTF-8
BoundedBytes<L>    arbitrary bytes
BoundedCanonicalBytes<L> validated canonical codec bytes
BoundedOpaqueBytes<L> no semantic parsing by consumer
BoundedTextOrBytes tagged text | bytes; encoding is never guessed
```

Decoders enforce limits before full allocation where framing permits it. Recursive anchors use a named
depth limit.

## Opaque and display wrappers

| Type | Visibility | Rule |
|---|---|---|
| `OpaqueId` | SharedDomain | non-empty bounded identity, no consumer parsing |
| `OpaqueRef` | SharedDomain | bounded reference; possession grants no authority |
| `OpaqueCanonicalBytes` | SharedDomain | producer-validated canonical bytes |
| `OpaqueHandleToken` | ProviderWire | CSPRNG bearer locator, redacted, current auth required |
| `BoundedDisplayName` | ProviderWire when authorized | display only, never identity |
| `BoundedDisplayPath` | ProviderWire after disclosure check | never authorization/identity |
| `BoundedName` | SharedDomain | normalized subject name |
| `BoundedSymbolKey` | SharedDomain | normalized exact/qualified symbol key |
| `BoundedExpression` | SharedDomain | descriptive predicate, never executable |
| `BoundedObservation` | ProviderWire | content-minimized observation |
| `BoundedBehaviorSignature` | SharedDomain | deterministic descriptive comparison signature |
| `BoundedNonContentMetadata` | ProviderWire | closed scalar metadata from `SUPPORT_SCHEMAS.md` |
| `BoundedNonContentRankingTrace` | ProviderWire | closed ranking trace from `SUPPORT_SCHEMAS.md` |
| `OpaqueAuthorizedFacetValue` | ProviderWire | resolved only after authorization |

## Identity and reference registry

Every entry is a distinct newtype/tagged union.

```text
InstallationId, InstallationIncarnationId, DataRootId, BindingId,
WorkspaceId, WorkspaceViewRevisionId, RootBindingId, PathBindingId,
RepositoryLineageId, CollectionGenerationId, CorpusId, ReferencePortfolioId,
SourceNamespaceId, SourceId, SourceMembershipId, ProjectionMembershipId,
SourceRevisionId, MaterializationId, RepresentationId, UnitId,
AccessPartitionId, ScoringPartitionId, ScoringDocumentId,
AccessPolicyBindingId, ResidencyPolicyBindingId, ScopeDomainId,
AccessDomainId, ConfidentialityDomainId, EncryptionKeyDomainId,
RetentionDomainId, ErasureDomainId, GrantId, RequestId, PlanId,
CandidateId, CutoverId, BufferSnapshotId, ImportedSnapshotId,
HandleId, ContinuationId, PublicationIntentId, PublicationReceiptId.
```

Counter/version wrappers:

```text
OwnerEpoch, Epoch, NonZeroRevision, PortfolioRevision,
CollectionRouteRevision, CatalogRevision, MembershipRevision,
AccessPolicyRevision, ShadowFenceRevision, PurgeFenceRevision,
ObservationCursorRevision, OverlayRevision, PolicyRevision.
```

Digest/profile wrappers:

```text
Blake3Digest32, Sha256Digest32, VersionedContentDigest,
SourceOwnerGeneration, ObjectResidencyKeyDigest, PlanFingerprint,
QuerySnapshotFingerprint, ArtifactDigest, DigestRef, HandleTokenDigest,
ProfileId, ProjectionProfileSetId, FusionProfileId, RuleId,
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

Exactly one union variant is present.

## Baseline semantic registries

```text
AssuranceClass = exact_bytes | mapped_text | lossy_text | descriptive_only
ObservationFreshnessState = current_confirmed | observed_with_age | gap_detected | unknown
EvidenceRole = definition | reference | test | documentation | caller | configuration
Modality = code | text | document | image | archive | mixed
```

`EntityKind` is a versioned registry, not arbitrary text:

```text
function, method, type, trait, impl, module, field, constant, static,
macro, variable, parameter, file, section, test, document, table,
image_region, unknown
```

Provider-specific subkinds are private/profile-qualified and cannot affect access or exact identity
without a new contract.

## Coverage records

```yaml
LegDescriptor:
  leg_ref: OpaqueId
  leg_kind: direct | exact | structural | lexical | semantic | rerank
  scoring_partition_ref: OpaqueRef | null
  profile_id: ProfileId

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
  state: ObservationFreshnessState
  observation_cursor_revision: ObservationCursorRevision
  observed_age_ms: u64 | null
```

Counts/scopes are authorization-filtered. Gap records contain no content, secret, inaccessible name or
absolute path.

## Port support records — owned by `search-ports`

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

These are `PortSupport`, not `search-contracts` provider records. Cancellation/stream refs are
process/package capabilities and non-serializable. No executor/channel/file/socket/vendor type appears.

## New-type rule

A new helper type request classifies owner, visibility, canonical representation, bounds, disclosure
and serialization. Local aliases duplicating this registry are forbidden.
