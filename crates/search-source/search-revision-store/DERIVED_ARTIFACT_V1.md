# C07 derived artifact v1 core (#270 / #260)

SCOPE_FROZEN: base 827999db0fa8c59b3b1302542599264bc50e3a4a; one owner,
`crates/search-source/search-revision-store/**`. State/effect: versioned opaque
derived descriptors, exact stored-byte CAS identity, four-object reference graph,
and verification-gated reuse of the existing contextual immutable I/O kernel.
Public API: additive `derived_artifact` module only. Immediate consumers are the
parent's C00/C02/Crypto/Windows and same-root daemon adapters, not changed here.
Persisted disposition: VERSIONED_REBUILD; raw v1/v2 bytes/formulas remain unchanged.
No legacy API is removed; legacy removal stays with its existing consumer owner.
Minimum gates: package all-features check, strict lib Clippy, one pure encoded-byte
identity/mismatch test. Native/live/credential/product qualification is NOT_RUN.
No remote issue/PR write, descendant, source reread or new authority is needed.

## Exact records and identities

`DerivedArtifactDescriptorV1` carries an exact `SourceRevisionRef` as provenance,
root-supplied RFC4122-v4 `MaterializationId` and `RepresentationId`, the producing
owner's real profile ID/revision/fingerprint, its semantic object digest, original
root operation, the six existing `DomainBinding` values, and direct BLAKE3/length
of the **actual encoded body bytes**. No digest is padded into a UUID. UUID shape
checking cannot prove issuance; `verify_storage_scope` must join the actual root.
Kinds are representation-body, materialization-metadata, unit-manifest,
projection-manifest and C07 set-manifest. The profile never appends a source
history occurrence, reuses `PreSealDescriptor`, or aliases raw objects.

Domain order and validation reuse `residency_canonical_bytes` unchanged: scope,
access, confidentiality, encryption-key, retention, erasure. Same-key reuse
requires `require_same_residency` (including every genuine UUID, canonical body
and tagged digest), followed by the actual selected-key/C02 owner check. A UUID
match, plaintext equality or a body digest alone is insufficient.

Let D be BLAKE3 of the exact stored object bytes. The new residency key is
`blake3_canonical("eliot/cbor/derived-artifact-residency-key/v1",
[1, complete_residency_canonical_bytes, "blake3-256", D])`. The path beneath the
actual admitted CAS root is `<key>/derived-artifact-v1-<kind>/<D[0..2]>/<D>`.
For metadata-only objects D hashes their exact owner encoding. For a sealed
object D hashes the complete ELSDRV01 envelope, never the source digest or
plaintext-body digest. The kind is a separate closed path component.

`DerivedArtifactSealBindingV1` emits canonical AAD with explicit derived AAD and
envelope v1 markers, unchanged `aes-256-gcm/tag128` and
`zero32-counter64/attempts32-blocks32-bytes36/v1` policy identities, full descriptor,
full selected Current record, exact key reference/version, external counter and
external nonce. Ciphertext-derived identity is absent from pre-effect AAD.
The GHASH charge remains `ceil(P/16) + ceil(AAD/16) + 1`.

ELSDRV01 is: magic[8], version u16 BE=1, AAD length u32 BE, ciphertext/tag length
u64 BE, nonce[12], canonical AAD, ciphertext/tag. Ciphertext/tag length is P+16.
The existing AES primitive and nonce policy are neither implemented nor changed
by C07. Raw ELSREV01/ELSREV02 decoders never accept this profile.

Every candidate and decoded descriptor/reference is UNQUALIFIED. In particular,
framing random bytes as ciphertext does not qualify them. Plaintext representation
body candidates cannot enter the metadata-only constructor. Technical metadata
also requires actual producer schema verification before it can reach a file.
Plaintext observations from the opener are zeroized and never written or returned
by C07 I/O.

`DerivedArtifactSetDescriptorV1` contains exactly the four non-set kinds in order,
with one exact source/materialization/representation and identical full residency.
Its C07 schema fingerprint and semantic commitment are owner-computed separately
from the encoded set-body digest. Publishing/reopening a set opens and verifies
all four exact members under the same root/context; nested sets are impossible.
These are object-graph edges, not a new catalogue, index or query language.

## Actual I/O and required parent ports

`publish_derived_artifact_v1` and `read_derived_artifact_v1` use the existing
`LegacyRevisionObjectContext` / `ContextualLegacyRevisionObjectPlatform` by type
alias. Private CAS entrypoints enter the same `Kernel`, with real residency/kind/
shard components. The kernel validates the supplied root and each directory,
charges original-context locator/native/I/O work, creates no-clobber staging,
syncs, compares exact staging/final bytes and identities, and preserves possible
effects and stages on failure. No second store or copied I/O implementation exists.
Descriptor/AAD work prepays conservative logical copy/processing bounds (16 times
the respective fixed metadata ceiling); set decoding prepays 16 times its actual
encoded length. Exact body hashing, I/O and producer work charge separately through
the same original context. These are logical admission bounds, not CPU/RSS proof.
The actual root adapter must hold its original owner guard and qualify directory
containment/durability. An I/O receipt proves this call's exact physical observation,
not continuing policy, stage custody, retention or visible publication.

There is intentionally **no implementation/default/stub** of
`DerivedArtifactVerificationPortV1<C>` here:

- `verify_storage_scope(context, cas_root, object_ref, Read|Publish)` joins the actual root,
  genuine source/materialization/representation IDs, policy/purge and all six
  domain bodies. Before sealed publication it joins the exact C02 late object
  intent with this ref/address/actual stored-envelope digest and length. It
  enforces metadata plaintext permission. Reads call it before
  file access, and all objects are rechecked before returning verified content.
- `verify_owner_payload(context, descriptor, exact_encoded_body)` invokes the
  actual C08/C09/C13 producer verifier, compares its real profile identity and
  semantic digest, verifies parent provenance, and rejects bodies in metadata
  kinds. C07 directly validates its own set schema and exact member readbacks.
- `open_authenticated(context, binding, ciphertext_and_tag)` is implemented by
  the actual Windows/Crypto owner. It joins committed C02 artifact reservation,
  selected Current/key and full residency, verifies the unchanged nonce/accounting
  policy and full AAD, and authenticates/decrypts. C07 hashes/compares the returned
  `DerivedArtifactPlaintextV1` independently. All internal work borrows the original
  caller context and its zeroization obligation survives refusal.

The smallest C02 addition is a versioned **derived subject** reservation and late
object-intent variant in the same journal, using these actual C07 descriptor/AAD
bytes (descriptor <=16 KiB, Current <=8 KiB, AAD <=32 KiB). It must atomically burn
the **same per-key usage-state counter/blocks/bytes** used by raw selected v2,
retain the original operation and full bytes, and return a C02-owned non-Clone
`CommittedDerivedArtifactSealUse` only on a fresh acknowledged commit. Replay,
recovery and unknown outcome issue no permit; release/purge do not reset usage.
The Windows seal operation consumes that permit once, validates the binding and
reserved charge, and lends the existing selected key to the unchanged AES256-GCM
primitive. It returns ciphertext/tag for `DerivedArtifactPayloadCandidateV1::sealed`.
It must not call a raw SourceRevision finalizer. C07 issues no counter, key, permit,
selection, credential, `ReceiptRef` or synthetic `published=true` result.

After AES, the C02 late intent must bind the actual object-ref encoding/address
and its exact stored-envelope digest/length before physical publication. C07
returns only an actual shared-kernel write/read receipt with private construction.
The parent derives original operation/phase IDs and owns the cross-owner link.

The same-root ingestion/publication caller retains actual `PublicationGuards`,
root guard and ID reservations. It persists artifact-set/source references through
the typed `PersistentControlJournal` publication intent and `VisibleEpochCommit`
and keeps them as S28 mark roots before Qdrant mutation. This package neither
patches those common contracts nor substitutes FileJournal/PublicationFloor.
Until the parent supplies the real verification/seal producers and that root
composition, this is a C07 profile/lifecycle core, not end-to-end qualification.
