# Function contract — `search-os-secrets`

**Status:** finite secret lifecycle and pure legacy revision-envelope compatibility contract are
implemented; qualified Windows effect composition and side-channel evidence remain separate work.

This package owns opaque OS-user/installation/incarnation/purpose-bound local secret lifecycle. Public
APIs never serialize plaintext secret bytes. Process launch, provider pairing, transport sessions,
platform I/O and source access remain outside this package.

## Global rules

- configuration and durable records contain only typed opaque `SecretRef` values;
- plaintext is available only inside a bounded guarded-use callback/lease adapter and is non-serializable;
- one reference binds OS user, installation, incarnation, purpose and authoritative generation;
- Debug, Display, serde, errors, receipts, logs, metrics and panic text expose no secret material;
- no secret enters command line, repository file, ordinary environment snapshot or crash metadata;
- deletion claims logical unavailability from this store, not physical secure erasure;
- pure compatibility codecs perform no filesystem, credential, DPAPI, RNG, source-catalog or policy I/O.

## Configuration operations

### `section_descriptor() -> Result<ConfigSectionDescriptor, ConfigError>`

Revision 2 owns exactly backend, lease TTL, rotation grace and the optional Qdrant opaque reference.
Defaults are `windows_user_bound`, 30000 ms, 60000 ms and Absent. TTL is inclusive 1000..300000;
grace is inclusive 0..600000. Backend/TTL/grace allow File only and no explicit reset; the reference
allows File/whitelisted Environment and explicit reset. CLI overrides are refused. Text/reference
and list bounds preserve the daemon profile: 4096 bytes, 64 items, 512 bytes per item.

The complete field-registry fingerprint uses shared canonical CBOR/BLAKE3 under
`eliot/cbor/secrets-section-descriptor/v2`: section, owner, revision, section reload floor,
secret policy, then fields in key order. Each field includes key, kind, typed default, all five
bounds, allowed sources, reset policy, security floor, redaction and independent actions. The
profile domain also fixes directional reload semantics. No private hash or generic builder is added.

### `validate_secrets_section(&ValidatedSection) -> Result<ValidatedSecretsConfig, ConfigError>`

Checks section/owner/revision/registry digest, exact four-field inventory, scalar bounds, fixed backend
and source/reset provenance. Defaults must equal the owner defaults; reset values must equal their
default. Captured revision-1 descriptors fail StaleDescriptor. Explicit opaque refs remain valid in
DIRECT. Reference grammar stays owned by `search-config::SecretReference`; no reference is resolved.
The environment whitelist and captured-source acquisition remain in their existing owners.

`ValidatedSecretsConfig` has private fields and getters `backend() -> &str`, `lease_ttl_ms() -> u64`,
`rotation_grace_ms() -> u64`, and `qdrant_api_secret_ref() -> Option<&SecretReference>`.
Debug is redacted. Construction is possible only through owner validation, without an unchecked
compiled-default factory. Descriptor defaults flow through the existing generic configuration builder.

### `section_digest(&ValidatedSecretsConfig) -> Blake3Digest32`

Retains the existing config-owned section digest over exact typed values and provenance, after owner
checks. It proves equality only, not runtime application, active configuration revision, backend
qualification, clock, lease availability or Current-key authority.

### `plan_section_change(&ValidatedSecretsConfig, &ValidatedSecretsConfig) -> Result<SectionDelta, ConfigError>`

TTL decrease contributes ApplyLive and is restrictive; increase contributes RestartDependency.
Grace changes contribute ApplyLive; reference changes contribute RestartDependency. Every value
change also retains the section RestartDependency floor, preserving independent actions. Equal values,
including provenance-only digest changes, require no action. Plans contain no application receipt and
never execute a live/restart operation. Runtime application and qualified key selection remain separate.

## Reference and binding operations

### `validate_binding(binding, current_platform_identity) -> Result<ValidatedSecretBinding, SecretError>`

Requires exact OS user identity, installation/incarnation, non-empty closed purpose and supported backend.
Cross-user/incarnation/purpose substitution fails closed.

### `validate_reference(reference, expected_binding, expected_purpose) -> Result<ValidatedSecretRef, SecretError>`

Checks reference schema/backend/store identity/generation and binding digest without opening plaintext.
Unknown versions and default/foreign refs are rejected.

### `reference_metadata(reference) -> RedactedSecretMetadata`

Returns backend class, purpose, generation, binding digest and lifecycle state only.

## Secret lifecycle

### `EncryptedPayload::new(bytes, max_bytes) -> Result<EncryptedPayload, SecretError>`

Rejects empty or over-budget ciphertext before hashing, then uses the shared
`search-contracts::blake3_bounded_bytes` with `Blake3ByteBudget::new(max_bytes)` to cache
raw BLAKE3 over exactly the owned bytes. Both bytes and digest fields are private; no
digest setter or unchecked constructor exists. Existing custom finite ciphertext ceilings,
including ceilings above the separate source helper's 16 MiB profile, remain valid.
The byte budget grants no allocation, admission, total-work or backend authority.

### `SecretRecord::new_active(reference, ciphertext, record_revision, operation) -> SecretRecord`

This const constructor copies the payload-owned digest into the record. It accepts no caller
ciphertext digest. `SecretCatalog::create` retains its existing capacity, payload and operation
checks; constructing a record proves no encryption or durable backend effect.
Deletion clears both payload bytes and their cached digest to the standard empty-input value;
the record's admitted ciphertext digest remains historical metadata. A cleared payload cloned
from a tombstone cannot be re-admitted because catalog admission rejects empty ciphertext.

### `SecretCatalog::prepare_rotation(id, binding, replacement_version, replacement_ciphertext, replacement_record_revision, operation)`

Returns `Result<SecretMutationEffect, SecretError>`. The pending rotation and write effect take
their expected digest from the validated replacement payload before lifecycle mutation; no
replacement digest is accepted as input. Existing binding/version/revision/state checks remain.
`confirm_rotation` and `recover_rotation` still require the exact observed digest, reference,
record revision and operation plus a durable receipt. Backend readback must observe actual
stored bytes independently; a derived payload digest supplies equality, not durable selection
or trusted readback provenance. Those obligations remain separate under #307.

### `create(binding, purpose, entropy_port, store_port, operation, deadline, cancel)`

Returns `Result<SecretCreateReceipt, SecretError>`.

Generates accepted-strength random material through an injected OS/CSPRNG port and durably stores it under
the validated binding. Success returns a new opaque reference plus content-free receipt.

Cancellation before store mutation is clean. Timeout/cancellation after possible write is
`SECRET_CREATE_OUTCOME_UNKNOWN`; recovery queries exact operation/binding/purpose identity and never
creates an untracked second secret.

### `with_secret(reference, expected_binding, purpose, consumer, deadline, cancel)`

Returns `Result<SecretUseReceipt, SecretError>`.

Opens plaintext only inside a non-serializable guarded consumer invocation. The consumer receives a
borrowed bounded secret view whose type cannot be returned, cloned into a public record or formatted.

The guard verifies binding/generation before and after use, zeroizes owned transient buffers where the
qualified backend/runtime permits and returns only operation/purpose/generation/timing-class metadata.

Cancellation before consumer invocation is clean. Cancellation during use signals the consumer but does
not claim immediate physical zeroization of provider/runtime copies; the receipt reports completion or
incomplete guarded use without secret data.

### `issue_lease(reference, expected_binding, purpose, ttl, consumer_identity) -> Result<SecretLease, SecretError>`

Creates a short-lived non-serializable lease capability for a daemon-owned adapter. TTL and purpose are
bounded by config/profile. A lease cannot be converted to plaintext by public API, serialized across a
process boundary or reused after incarnation/generation change.

### `rotate(reference, expected_binding, operation, deadline, cancel) -> Result<SecretRotationReceipt, SecretError>`

Implements a crash-recoverable state machine:

```text
CURRENT(G)
→ NEXT_STORED(G+1)
→ AUTHORITY_SWITCHED(G+1)
→ OLD_RETIRED(G)
```

At every recovery point exactly one generation is authoritative; bounded grace may permit explicit
verification of the retiring generation but cannot make both default. Same operation is idempotent and
conflicting input rejects.

### `delete(reference, expected_binding, operation, deadline) -> Result<SecretDeletionReceipt, SecretError>`

Revokes future resolution, invalidates outstanding leases for the reference/generation and requests
backend deletion. Success is idempotent and explicitly does not claim physical secure erase, backup
removal or platform forensic erasure.

### `recover_operation(operation, expected_binding, store_port) -> Result<SecretRecoveryDecision, SecretError>`

Resolves unknown create/rotate/delete outcomes by exact backend operation metadata and generation state.
Ambiguous or corrupt state locks/quarantines the reference rather than exposing/choosing material.

## Revision DEK candidate metadata

`RevisionDekBinding::new(DataRootId, InstallationId, InstallationIncarnationId,
[u8; 32] user_scope_digest, [u8; 16] logical_operation_id)` creates pure metadata with
private fields/getters. Zero identifiers/digest/operation refuse. Reference derivation uses
`eliot/cbor/revision-dek-reference/v1` over the canonical binding array with the reference
field omitted. The full 11-field array is root, installation, incarnation, user digest,
logical operation, computed reference, planned key version 1, record revision 1,
`revision-aes256-gcm-dek`, `windows-user-dpapi-credential.v1`, profile revision 1.
`binding_digest()` hashes that array under `eliot/cbor/revision-dek-binding/v1`;
`protection_entropy()` returns its exact canonical CBOR bytes. The three digest getters
return `[u8; 32]`; entropy is an owned `Vec<u8>`.

`hash_native_revision_user_sid(&[u8]) -> [u8; 32]` hashes a canonical CBOR byte string under
`eliot/cbor/windows-user-scope/v1`. Its input must be at most 68 bytes; a larger input
panics before encoding. This helper neither validates SID structure nor observes an OS user.

`RevisionDekIntent::new(binding, OwnerEpoch, protected_blob)` computes raw BLAKE3 and length
from actual nonempty protected bytes, accepting at most 2516 bytes (2560 minus frame header 44).
It retains no blob or key. `RevisionDekCandidate::new(&intent, &expected_binding, actual_blob)`
rechecks exact protected bytes and scope and retains the original historical creator epoch.
That epoch is separate from present Native authority and the planned version/revision.

`encode_revision_dek_record(&RevisionDekRecord)` and `decode_revision_dek_record(&[u8])`
own the strict canonical 7-field array: `eliot.revision-dek.record.v1`, codec 1,
kind 0 Intent or 1 CandidateNonServing, binding array, creator epoch, protected digest,
protected length. The 1024-byte metadata ceiling, exact field counts/types/byte lengths,
computed reference, fixed purpose/backend/versions, positive epoch and blob bounds are checked.
Unknown, trailing, noncanonical and truncated records refuse through content-free errors.

Public construction/decoding proves metadata equality only. Native must validate actual user,
root custody, wrap/unwrap and protected readback; C02 must conditionally commit/read back the exact
intent/candidate before acknowledgement. Decoded candidate metadata grants no backend-existence
proof, Current selection, key-use/lease, rotation, deletion or C07 encryption authority.
Key, SID and credential coordinates are absent from public Debug/errors; protected bytes never
enter these control records. Legacy lifecycle and compatibility codecs remain separate.

## Revision DEK first-Current application metadata

`RevisionDekSelectionPolicy::from_config(&ValidatedSecretsConfig, ConfigFingerprint)` returns
`Result<RevisionDekSelectionPolicy, RevisionDekSelectionError>`. It requires the exact BLAKE3 /
`eliot/cbor/config-snapshot/v2` / revision-2 tag, the source-admitted `windows_user_bound` backend,
nonzero config/section digests and the current owner descriptor registry identity. Getters `backend()`,
`config_fingerprint()`, `secrets_section_digest()` and `registry_digest()` return the fixed backend
string and three `[u8; 32]` equality identities. Decoding retains bytes under this fixed profile;
it does not reconstruct a typed ConfigFingerprint or restore/apply the whole configuration.

`expected_backend_identity(&RevisionDekCandidate) -> Result<[u8; 32], RevisionDekSelectionError>`
hashes `[canonical_candidate_v1_bytes, backend, application_scope]` through the shared canonical owner
under `eliot/cbor/revision-dek-backend-identity/v1`. This is equality metadata, never Native evidence.
The sole application scope is `revision-dek-current-selection/windows-user-bound/v1`. TTL, rotation
grace and the Qdrant reference remain unapplied; no lease, key use or revision encryption is exposed.

`RevisionDekApplicationIntent::new(RevisionDekCandidate, [u8; 16], RevisionDekSelectionPolicy,
OwnerEpoch)` returns a fallible pure metadata value with private fields. Getters expose `candidate()`,
`operation_id()`, `policy()`, `creator_epoch()` and `request_digest()`. A nonzero distinct-purpose
selection operation and exact existing candidate are retained. Request hashing uses the complete
seven logical intent fields below under `eliot/cbor/revision-dek-selection-request/v1`, excluding
only the application-intent creator epoch. The candidate's immutable historical epoch remains in its
unchanged canonical bytes. A fresh invocation's Native custody is never represented by replay identity.

`RevisionDekCurrent::new(&RevisionDekApplicationIntent)` plans metadata only. `intent()`,
`applied_policy_revision()` and `selection_revision()` expose the original intent and planned initial
revisions 1/1. These axes are distinct from key version, descriptor revision, fingerprint format,
Native epoch and journal generation. Only Root's actual Windows dependency readback plus an exact
absent-state C02 CAS and operation readback can establish durable application/Current publication.
Constructing or decoding this type does not establish any of those effects.

`RevisionDekSelectionRecord::{Intent, Current}` shares `intent()`. The fallible
`encode_revision_dek_selection_record` / `decode_revision_dek_selection_record` codec has an
8192-byte finite ceiling and exact canonical four-array framing:
`["eliot.revision-dek.selection.v1", 1, kind, body]`. Intent kind 0 has body
`[selection_op16, canonical_candidate_v1_bytes, config_v2_digest32, secrets_section_digest32,
descriptor_registry_digest32, "windows_user_bound", application_scope, intent_creator_epoch]`.
Current kind 1 has body `[intent_body, 1, 1, expected_backend_identity32]`. Candidate bytes are
encoded/decoded only by the existing candidate owner; Candidate v1 and its original Intent are never
relabelled or rewritten. Fixed profiles/versions, nonzero identities, positive historical epoch,
exact lengths/counts, expected backend identity and canonical/trailing/truncation framing refuse
closed. Debug/errors disclose no protected bytes, credential coordinates, raw references or paths.
Exact C02 record/command bytes remain separately bound by the journal command identity.

## Legacy revision compatibility envelope

These functions preserve the frozen DIRECT protected-object format while platform effects remain in
qualified adapters. They create no source authority and do not decide whether plaintext may be admitted.

### `encode_legacy_revision_inner(binding, plaintext) -> Result<Vec<u8>, LegacyRevisionEnvelopeError>`

Emits the exact `ELSIN2\0\0` version-1 authenticated inner layout. Plaintext length must equal the
immutable binding and remain at or below 64 MiB. The caller must validate the content digest before
platform encryption.

### `decode_legacy_revision_inner<D>(inner, binding) -> Result<&[u8], LegacyRevisionEnvelopeError>`

Requires exact inner magic, version, namespace, key binding, revision identity, content digest and length.
`D` is the injected concrete digest owner; the package does not choose or relabel a hash implementation.

### `encode_legacy_revision_outer(binding, protected) -> Result<Vec<u8>, LegacyRevisionEnvelopeError>`

Emits the exact `ELSRV2\0\0` version-1 outer layout around non-empty protected bytes, with the complete
object bounded at 65 MiB.

### `decode_legacy_revision_outer(object, expected, expected_key)`

Returns `Result<(LegacyRevisionBinding, &[u8]), LegacyRevisionEnvelopeError>`.

Checks exact framing, payload length, namespace, revision identity, content binding and optional qualified
platform-key binding. Returning borrowed protected bytes is not decryption evidence.

### `legacy_revision_is_protected_object(bytes) -> bool`

Checks the outer format marker only. Marker presence never proves authentication, current policy,
decryption or plaintext admission.

## Health and audit

### `health(reference_or_backend, expected_binding) -> SecretStoreHealth`

Returns availability/locked/quarantined state, backend class and redacted generation counts. It contains
no existence details for foreign bindings.

### `audit_public_surfaces(fixtures) -> SecretSideChannelAudit`

Package-local test operation scans Debug/Display/serde/error/receipt/log/metric/process-launch fixture
surfaces for canary bytes. It is evidence support, not a production secret scanner.

## Cancellation, deadline and crash semantics

All store operations use finite deadlines and stable mutation identities. A timeout after possible write
is unknown until exact recovery. Crashes never permit fallback to another user/incarnation/purpose or
return secret material in recovery diagnostics. Pure envelope operations have no external mutation and
are deterministic over finite inputs.

## Typed failures

- `SECRET_BACKEND_UNAVAILABLE`
- `SECRET_BACKEND_NOT_QUALIFIED`
- `SECRET_REFERENCE_INVALID`
- `SECRET_BINDING_MISMATCH`
- `SECRET_PURPOSE_MISMATCH`
- `SECRET_GENERATION_MISMATCH`
- `SECRET_STORE_LOCKED`
- `SECRET_QUARANTINED`
- `SECRET_LEASE_EXPIRED`
- `SECRET_LEASE_REVOKED`
- `SECRET_CREATE_OUTCOME_UNKNOWN`
- `SECRET_ROTATION_INCOMPLETE`
- `SECRET_DELETE_OUTCOME_UNKNOWN`
- `SECRET_OPERATION_CONFLICT`
- `SECRET_USE_CANCELLED`
- `SECRET_PLAINTEXT_FORBIDDEN`
- `SECRET_LEGACY_REVISION_PROTECTED_PAYLOAD_INVALID`
- `SECRET_LEGACY_REVISION_PLAINTEXT_TOO_LARGE`
- `SECRET_LEGACY_REVISION_PROTECTED_FORMAT_REQUIRED`
- `SECRET_LEGACY_REVISION_ENVELOPE_INVALID`
- `SECRET_LEGACY_REVISION_NAMESPACE_MISMATCH`
- `SECRET_LEGACY_REVISION_KEY_BINDING_MISMATCH`
- `SECRET_LEGACY_REVISION_BINDING_MISMATCH`
- `SECRET_LEGACY_REVISION_INNER_ENVELOPE_INVALID`
- `SECRET_LEGACY_REVISION_INNER_BINDING_MISMATCH`
- `SECRET_LEGACY_REVISION_LENGTH_MISMATCH`
- `SECRET_LEGACY_REVISION_CONTENT_MISMATCH`

## Required tests / qualification evidence

- public type/serde/Debug/error/receipt surfaces cannot expose plaintext;
- canary absent from argv, config, environment snapshot, logs, metrics and crash metadata;
- cross-user, cross-installation, cross-incarnation and cross-purpose resolution denied;
- reference schema/backend/store/generation mismatch fails closed;
- create crash/timeout before/after backend write and receipt publication;
- rotation crash at every state yields one authoritative generation;
- old lease invalid after rotation/deletion/incarnation change;
- lease TTL/purpose/consumer and non-serialization bounds;
- deletion idempotency and explicit secure-erasure nonclaim;
- backend lock/corruption produces locked/quarantined state without existence leakage;
- legacy envelope frozen bytes, truncation, field mutation, wrong namespace/key/revision/content/length and
  oversize/empty protected payload fail closed;
- `secrets` configuration plaintext/ref/change-policy fixtures;
- fake entropy/store/clock/consumer ports prove no process/session ownership.
