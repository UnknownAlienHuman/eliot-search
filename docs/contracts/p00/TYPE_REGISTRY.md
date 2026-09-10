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

## P00 clarification #48 — `UtcTimestamp`, `MetadataKey`, `UnresolvedSource`

Additive schema-v1 closure for Issue #48. Wire is unchanged; this section only closes the three
shapes exactly as proposed. Unknown or off-shape values fail closed.

### `UtcTimestamp`

Closed scalar UTF-8 timestamp. Shape owner: `search-contracts`; wall-clock production:
`search-ports` `ClockPort::utc_now`; meaning/time-ordering consumers: every package holding a
timestamp field plus `search-domain` transitions that compare them.

```yaml
UtcTimestamp: canonical_string # exactly 27 bytes
```

Wire form:

- canonical RFC 3339 UTC form `YYYY-MM-DDTHH:MM:SS.ffffffZ`;
- exactly 6 fractional digits (fixed total length 27; separators at `4:'-'`, `7:'-'`, `10:'T'`,
  `13:':'`, `16:':'`, `19:'.'`, `26:'Z'`; every other position is an ASCII digit);
- only `Z` suffix; offsets such as `+00:00` are rejected;
- no leap second: second value `60` is rejected (`second <= 59`, `minute <= 59`, `hour <= 23`);
- valid proleptic-Gregorian calendar date: `01 <= month <= 12`, `01 <= day <= days_in_month(year,
  month)` including the February 29 leap rule (divisible by 4, except centuries unless divisible
  by 400);
- year `0000` is rejected (`year >= 0001`).
- JSON: canonical string; CBOR: the same textual value (`CanonicalValue::Text`).

Invariant: lexicographic (bytewise) order equals chronological order. The form is fixed-width, so
derived string ordering is chronological ordering.

Producers: `ClockPort::utc_now` and every record constructor embedding a timestamp
(`SourceOwnerCutover.prepared_at/effective_at`, `CutoverAuthorization.issued_at`,
`SourceRevision.observed_at`, `SearchReadGrantClaims.issued_at/expires_at`,
`SearchTaskPlan.created_at/expires_at`, `EmissionSecurityFence.checked_at`,
handle/continuation `created_at/expires_at`, `HandlePermit.expires_at`).

Consumers: all decoders through the single validating parser (fail-closed on any deviation),
ordering/comparison checks (`effective_at >= prepared_at`, `expires_at > issued_at/created_at`),
and canonical JSON/CBOR encoders. `plan_id`, timestamps and the fingerprint itself are excluded
from `PlanFingerprint` unless the accepted canonical fixture explicitly treats `plan_id` as a
deterministic digest projection (see `QUERY_AND_RESULTS.md`).

### `MetadataKey`

Closed canonical map key for `BoundedNonContentMetadata`. Shape owner: `search-contracts`;
scalar meaning owner: `search-domain`; map producers/consumers: every package emitting or
decoding `BoundedNonContentMetadata.entries`.

```yaml
MetadataKey: bounded_token # 1-128 bytes, UTF-8
MetadataKey_pattern: "[a-z][a-z0-9_.-]*"
```

Wire form:

- bounded UTF-8 token, 1–128 bytes (`metadata_key` bound class; empty and longer values are
  rejected);
- first byte `[a-z]`; remaining bytes each `[a-z0-9_.-]`; anything else (uppercase, `/`, space,
  `=`, non-ASCII such as `å`) is rejected;
- JSON string map key; CBOR text map key under the same bytes.

Invariant: canonical lower-case (already canonical on the wire; decoders perform no case folding
and reject uppercase); bytewise compare (`BTreeMap`/`BTreeSet` ordering); unique after
canonicalization (duplicate map keys and duplicate set items are rejected, never silently
normalized); carries no source content and no authorization material (content lives only in the
closed `MetadataScalar` variants in `SUPPORT_SCHEMAS.md`; authority lives in grants, fences and
receipts, never in a metadata key).

Producers: any producer of a `BoundedNonContentMetadata` entry. Consumers: all
`BoundedNonContentMetadata` decoders and canonical-map encoders, which enforce the token rule,
the 64-entry bound and key uniqueness before use.

### `UnresolvedSource`

Closed cutover-validation record. Shape owner: `search-contracts`; cutover-meaning owner:
`search-domain`; state/verification owners: `search-publication` intent state and
`search-ports` `SourceOwnershipPort::verify_cutover_receipt`.

```yaml
UnresolvedSource:
  source_id: SourceId
  reason_codes: bounded_set<SearchReasonCodeV1> # non-empty, P00 reason-code limit
```

Wire form:

- exactly the two fields above; no additional field is legal (closed object);
- `reason_codes` is non-empty (empty is rejected);
- `reason_codes` is bounded by the P00 reason-code limit: at most 64 items
  (`ContractBoundsV1` `reason_codes` class); set semantics apply — canonical uniqueness with
  deterministic iteration, duplicates rejected;
- each element is a closed `SearchReasonCodeV1` value from the exact wire registry in
  `REASON_CODES.md`; unknown codes fail closed and are never defaulted or coerced;
- carried on the existing field
  `CutoverValidation.unresolved_sources_and_reasons: bounded_list<UnresolvedSource>`; that
  field shape is unchanged.

Invariant: only machine-readable closed reasons. An `UnresolvedSource` carries no evidence
payload, no authorization decision, no display path, no raw bytes and no free-form text. Gap-only
reasons (`STALE`, `UNREADABLE`, `ACCESS_REVOKED`, `PURGED`, `SOURCE_REVISION_UNAVAILABLE`) keep
their `QUERY_AND_RESULTS.md` rule: they describe validation/coverage gaps and cannot label an
emitted evidence candidate.

Producers: cutover validation constructing
`CutoverValidation.unresolved_sources_and_reasons`. Consumers: cutover receipt verification and
any downstream planner, which must surface an unresolved entry as explicit incomplete/blocked
coverage rather than silent success or silent omission.

### Compatibility of the three closures

All three are additive closures of schema v1. Wire bytes, field names, enum spellings and bound
values are unchanged; no version bump and no migration. Existing canonical encoders already emit
the closed forms and existing decoders already reject everything outside them, so previously
accepted bytes remain accepted and previously rejected bytes remain rejected.

## New-type rule

A new helper type request classifies owner, visibility, canonical representation, bounds, disclosure
and serialization. Local aliases duplicating this registry are forbidden.
