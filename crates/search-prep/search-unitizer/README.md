# search-unitizer

Deterministic bounded unit occurrences and immutable manifests for an exact materialized representation.
The #257 implementation provides the `exact-unit-manifest/v3` UTF-8 text/code baseline. Source presence
and a nonzero fixture binding do not establish execution or product qualification; this documentation
update records the integrated implementation without claiming a test, build or qualification result.

## Accepted v3 baseline

`V3RepresentationKind::{Text, Code}` accepts nonempty, byte-identical UTF-8 materializations with
`ExactBytes` assurance and an empty loss map. `prepare_unit_set_input` is the sole materialization
ingress: it borrows an immutable `search-materializer` product, calls that owner's
`verify_materialization`, checks its request/profile and byte commitments, and converts the verified
scalar line coordinates to exact byte offsets. It performs no source-store or filesystem I/O.
`V3SourceBinding` supplies admitted namespace, source, revision, materialization and representation UUIDs;
the adapter checks the request source identity but does not admit these coordinates or derive them from
materializer digests.

The explicit baseline decisions are:

| Descriptor decision | Accepted value |
| --- | --- |
| Representation family | `Text` / `text` or `Code` / `code` |
| Unit kind | Contract `UnitKind::File`, `Section` or `Doc` |
| Boundary revision | `1`, reusing the existing exact UTF-8 layout algorithm |
| Overlap | `V3OverlapPolicy::None` / `none` |
| Anchors | `V3AnchorPolicy::ExactTextBytes` / `exact-text-bytes` |
| Attachments | `V3AttachmentPolicy::Absent` / `absent` |
| Omission | `V3OmissionPolicy::Forbidden` / `forbidden` |
| Empty input | `V3EmptyPolicy::Reject` / `reject-empty` |
| Minimum unit bytes / maximum anchor depth | `1` / `1` |

Empty input returns `UnitizationError::EmptyInput` (`UNITIZATION_EMPTY_INPUT`); `allow-empty` is an
unsupported profile tag. Lossy, transcoded, ambiguous and attachment-bearing profiles are outside this
baseline. Structural identities and configuration predicates must be absent and occupy explicit null
wire slots; the codec rejects populated slots. The unitizer does not infer compiler or predicate truth.

The shared layout prefers exact logical-line ends around the preferred byte size, splitting longer
lines only at safe UTF-8 boundaries within the hard byte limit. Units are contiguous, ordered and
nonempty; the final cursor must equal the exact input length. Scalar and per-unit line ceilings are
additional checks on the selected spans, with excess refused rather than a different layout invented.
Every byte is represented exactly once and `omitted_bytes` is zero.

## Public API and verified output

The implemented flow is:

```text
validate_v3_unitizer_profile(descriptor) -> ValidatedV3UnitizerProfile
prepare_unit_set_input(binding, product, request, materializer_profile, profile, budget) -> UnitSetInput
build_unit_manifest(input, profile, budget) -> VerifiedUnitSet
canonicalize_unit_manifest(set.manifest()) -> CanonicalUnitManifestBytes
decode_unit_manifest(bytes, max_encoded_bytes) -> UnitManifest
verify_unit_manifest(decoded, input, profile, budget) -> VerifiedUnitSet
```

Every fallible operation returns `Result<_, UnitizationError>`. `ValidatedV3UnitizerProfile` exposes
`descriptor()`, `id()`, `limits()` and `revision()`. `UnitSetInput` is opaque and tied to its validated
profile. `VerifiedUnitSet`, `UnitManifest` and `UnitDescriptor` have no caller-writable fields or public
constructors. `VerifiedUnitSet` exposes `manifest()`, `representation()`, `units()` and
`manifest_digest()`; `UnitDescriptor` exposes its contract `UnitOccurrence`, spans, line-boundary flags
and separate content/reference/identity digests. The output contains occurrence metadata, not source
text, paths, ranking scores or vendor payloads.

The returned contract `Representation` binds the typed representation/materialization IDs, validated
profile ID and exact manifest digest, with no enrichment profile IDs. Decoding checks the closed schema,
profile identity, body digest and canonical re-encoding, but returns a proposed manifest. Only a rebuild
against the prepared exact materialization can turn that stored manifest into a `VerifiedUnitSet`.
`UnitManifestVerificationReceipt` is an alias of `VerifiedUnitSet`, not an additional attestation.
Neither output proves filesystem currentness, admission, publication or Qdrant indexing.

`diff_unit_manifests` accepts verified sets and returns created/retained/retired contract `UnitId`s.
Retention requires equal full occurrence descriptors; the same ID with different descriptors fails
with `UnitizationNondeterministic`. `manifest_digest` returns the manifest's stored commitment and does
not independently verify it.

## Closed v3 wire schema

All v3 encoding, parsing and BLAKE3 operations delegate to the shared `search-contracts` canonical
codec/digest owner. The durable format is deterministic canonical CBOR, not the legacy binary format.
Object fields are closed and positional arrays have exact lengths; unknown, missing, mistyped or
unsupported fields are rejected. UUIDs are 16-byte byte strings, digests are 32-byte byte strings,
counts/offsets/revisions are unsigned integers and boundary flags are booleans.

The envelope is the four-field object:

```text
{ format: "exact-unit-manifest/v3", version: 3, body: <object>, digest: <32 bytes> }
```

The body has exactly `provenance`, `profile`, `profile_id`, `input_bytes`, `represented_bytes`,
`omitted_bytes`, `line_count` and `units`. The manifest digest covers the canonical body, excluding the
envelope digest field. The serialized profile is the full 18-field descriptor: `profile_name`,
`profile_revision`, `representation_kind`, `unit_kind`, `boundary_revision`, `overlap_policy`,
`anchor_policy`, `attachment_policy`, `omission_policy`, `empty_policy`, `limits`, `min_unit_bytes`,
`max_unit_scalars`, `max_unit_lines`, `max_anchor_depth`, `max_manifest_bytes`, `max_steps` and
`fixture_digest`. `limits` is a closed object containing `max_input_bytes`, `preferred_unit_bytes`,
`max_unit_bytes`, `max_lines` and `max_units`.

`provenance` is an exact ten-slot array, in order: the five-ID binding array; materializer commitment;
materializer profile digest; canonical, coordinate, loss and input digests; native byte count; canonical
byte count; legacy materializer revision sequence. The binding order is namespace, source, revision,
representation, materialization. Materializer commitments remain full digests rather than UUID casts.

Each `units` entry is an exact 16-slot array, in order: unit ID, representation ID, unit-kind text,
ordinal, native anchor, null structural identity, null configuration predicate, canonical byte start/end,
logical-line start/end, start/end line-boundary flags, content digest, reference digest, identity digest.
The native anchor is `["text-bytes", input_digest, byte_start_0, byte_end_exclusive_0]`. Byte and line
ranges are half-open. These metadata slots do not contain the unit's content bytes.

Legacy `ELSUMF01`/`ELSUMF02` binary manifests and unsupported format/version tags return
`UnitManifestLegacyUnsupported` (`UNIT_MANIFEST_LEGACY_UNSUPPORTED`). Rebuild from retained exact
materializations under an accepted v3 profile; relabelling an old manifest does not migrate it.

## Identity domains

| Shared digest domain | Input |
| --- | --- |
| `eliot/cbor/unitizer-profile/v3` | Full canonical profile descriptor, including every decision, budget and fixture binding |
| `eliot/cbor/unit-provenance/v3` | Canonical provenance record |
| `eliot/raw/unit-content/v3` | Exact unit-content bytes |
| `eliot/cbor/unit-reference/v3` | Provenance commitment, native anchor and canonical byte span |
| `eliot/cbor/unit-occurrence/v3` | Provenance/profile, kind/ordinal, byte/line bounds, boundary flags, anchor and content/reference digests, with null attachments |
| `eliot/cbor/unit-manifest/v3` | Complete canonical body |

The profile ID is contract `ProfileId` text `unitizer-v3-` followed by the full 64-digit digest hex.
The contract `UnitId` takes the first 16 occurrence-digest bytes exactly; the descriptor retains the
full occurrence digest separately. These are identities of occurrences in an exact
representation/profile. A repeated truncated ID with a different full commitment returns
`IdentityCollision` (`UNIT_ID_CONFLICT`); a repeated identical commitment is also refused.
The IDs carry no promise of semantic stability across revisions or reparses. Paths,
display text and ranking state are excluded. A nonzero `fixture_digest` binds a fixture set; validation
does not execute or qualify that set.

## Finite budgets

The v3 descriptor requires explicit limits; zero never means unlimited. Let `B` be
`search_contracts::MAX_CANONICAL_BYTES - 128`.

| Limit | Accepted bound |
| --- | --- |
| Input bytes, input logical lines, encoded manifest bytes | Positive, each at most `B` |
| Preferred unit bytes | Positive and at most `max_unit_bytes` |
| Maximum unit bytes | Positive and at most `max_input_bytes` |
| Unit count | `1..=4096` |
| Per-unit scalars | Positive and at most `max_unit_bytes` |
| Per-unit logical lines | Positive and at most both `max_lines` and `max_unit_bytes` |
| Accounted work steps | `1..=32_000_000` |

`profile_revision` is nonzero; the fixture digest must contain a nonzero byte. The legacy
`DEFAULT_UNITIZATION_LIMITS` is not a valid v3 default: its input and unit-count ceilings exceed the
accepted v3 bounds.

`UnitizationBudget` supplies positive `max_steps` and `max_encoded_bytes` no larger than the profile
ceilings, plus an absolute monotonic `Instant` deadline and a borrowed live `AtomicBool` cancellation
flag. Per-call deadlines, cancellation and narrowed budgets do not enter durable identities or change
successful canonical bytes. Preparation and construction check accounted work, cancellation/deadline
at scan checkpoints (each line, each 256-byte content chunk and before each output span),
and output size; refusal returns an error with no complete verified set. This is an accounted-work
contract, not a measured latency guarantee. Profile manifest limits cover the whole encoded envelope;
digest calls separately reserve 128 bytes for shared framing headroom.

## Compatibility and future scope

The legacy DIRECT profile APIs (`UnitizerProfileDescriptor`, `ValidatedUnitizerProfile`,
`UnitizerProfileId`, `validate_unitizer_profile`, `unitizer_profile_digest`,
`classify_unitizer_profile_change`), receipt-bound `unitize` and pure `unitize_text`/layout codec APIs
remain unchanged until the #331 consumer cutover. Their types and identities do not construct or verify
a v3 set. Pure layout may still accept empty text and an empty line inventory; that behavior does not
override the v3 reject-empty materialization contract. The legacy profile framing is confined to this
DIRECT compatibility surface; there is no retained v2 durable-manifest producer.

The wider [FUNCTIONS.md](FUNCTIONS.md) logical contracts remain future scope where not explicitly mapped
to this baseline: structural/predicate profiles, transformed or lossy anchor policies, generalized
request/boundary ports, batch execution and accounting, disclosure-specific views and independent v3
profile-transition classification. Such profiles require explicit acceptance and qualification rather
than silent weakening of the current schema. Source admission/storage, materialization production,
semantic enrichment, lexical/model encoding, ranking, publication and Qdrant transport remain outside
this package. See [AGENTS.md](AGENTS.md) for package ownership and the named materializer ingress
exception.
