# Canonical primitive and encoding rules

## Precedence correction

`SourceOwnerGeneration` is `Blake3Digest32`, as required by Architecture S7.2.1. The
`NonZeroU64` sketch in H3.1 is a subordinate handoff typo. `OwnerEpoch` remains `NonZeroU64`.

## Primitive wrappers

| Type family | Canonical rule |
|---|---|
| UUID-backed IDs | distinct newtype per identity; JSON lower-case hyphenated UUID; canonical CBOR 16-byte value |
| `OwnerEpoch` | non-zero unsigned 64-bit integer |
| `Epoch` | signed 64-bit integer, `0 <= value < i64::MAX` |
| revision/generation counters | dedicated unsigned integer newtype; zero allowed only when the owning schema explicitly defines an initial state |
| `Blake3Digest32` | 32 bytes; JSON 64 lower-case hex characters |
| `Sha256Digest32` | 32 bytes; JSON 64 lower-case hex characters |
| `SourceOwnerGeneration` | dedicated wrapper over `Blake3Digest32`, never an integer |
| opaque identity | non-empty bounded UTF-8 or canonical bytes with an explicit type and maximum length |
| raw bytes in JSON | base64url without padding; never implicit UTF-8 |
| timestamp | RFC 3339 UTC string with `Z`; canonical form uses exactly six fractional digits; reject offsets and non-canonical alternatives at load-bearing boundaries |
| duration/deadline | unsigned integer milliseconds; no floating-point duration |

The contract crate defines explicit bounds for every opaque string/byte/list field. P00 defaults are
256 bytes for opaque IDs, 1,024 bytes for opaque references and 8 MiB for a full provider frame unless
a narrower schema states otherwise.

## Required strong IDs

In addition to H3.1, P00 defines distinct wrappers for identities used by Part I schemas, including:

```text
DataRootId, BindingId, WorkspaceId, RootBindingId, PathBindingId,
MaterializationId, WorkspaceViewRevisionId, GrantId, RequestId, PlanId,
CandidateId, CutoverId, BufferSnapshotId, ImportedSnapshotId,
AccessPolicyBindingId, ResidencyPolicyBindingId, HandleId, ContinuationId,
PublicationIntentId, PublicationReceiptId, CollectionRouteRevision,
CatalogRevision, MembershipRevision, AccessPolicyRevision,
ShadowFenceRevision, PurgeFenceRevision, ObservationCursorRevision,
PolicyRevision, ProfileId, RuleId and ReceiptRef.
```

A UUID/string may not substitute for one of these at a public boundary.

## Versioned content digest

```yaml
VersionedContentDigest:
  algorithm: blake3_256 | sha256
  bytes: fixed_32_bytes
```

Native Search objects use `blake3_256`. Wire protocols that explicitly require SHA-256 compute it from
verified source bytes; they never relabel a BLAKE3 digest.

## Canonical serialization

- Provider transport: `u32` little-endian byte length plus UTF-8 JSON.
- Identity/fingerprint inputs: RFC 8949 deterministic CBOR over explicitly versioned structs.
- Map keys and enum discriminants are fixed by schema version; map iteration order is never observable.
- Floating point is forbidden in identity/fingerprint inputs. `PdfRegion` coordinates are result data,
  not identity input unless a future profile defines a fixed-point canonical mapping.
- JSON reserialization is never used to derive an identity.

## Domain separation

At minimum, implementations use distinct prefixes:

```text
eliot-search/source-owner-generation/v1
eliot-search/object-residency-key/v1
eliot-search/point-identity/v1
eliot-search/plan-fingerprint/v1
eliot-search/schema-identity/v1
eliot-search/receipt/v1
```

The prefix, schema version and canonical payload are all hashed. Golden-byte and digest fixtures are
required for each domain.

## Unknown fields

- Unknown load-bearing fields in security, scope, identity, budget, currentness, lifecycle and ownership
  records fail closed.
- Protocol minor-version negotiation may permit explicitly registered non-load-bearing extension fields.
- `deny_unknown_fields` alone is not sufficient: version-aware decoding and extension classification
  are part of the contract tests.
