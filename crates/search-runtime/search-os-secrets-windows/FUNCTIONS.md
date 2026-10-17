# Windows secret effects

The platform package owns current-user Credential Manager/DPAPI effects. Core owns
metadata codecs and secret policy; daemon/C02 own Native custody and durable roles.

## Revision-DEK selection

`construct_revision_dek_selection(policy, candidate, original_context)` reconstructs
only the `windows_user_bound` backend for
`revision-dek-current-selection/windows-user-bound/v1`. It uses existing
`read_revision_dek` for actual nonimpersonating SID/binding/frame/DPAPI validation,
then the Core candidate constructor to compare the exact observed protected bytes
against the retained candidate. Missing, mismatched or refused readback fails closed.
The original context meters the read and bounded metadata comparison; no budget is
renewed. Candidate-v1 bytes and candidate creation remain unchanged.

`RevisionDekSelectionDependency` privately retains the owned readback, exact policy
and candidate. It has no decode, clone or key export. Its
`prepare_current_record(intent)` rejects a different policy/candidate and delegates
planned result metadata to Core. Pure metadata planning neither commits revisions
nor establishes current Native custody. The caller keeps the dependency alive
through its fences and exact Current CAS/readback, and reconstructs it again for
any later present-backend availability statement. Invocation epochs are not matched
against the candidate's historical creator epoch.

The revision-DEK selection leaf performs no RNG, credential write/delete, lease, encryption, TTL/grace/ref
enforcement or whole-daemon activation. Protected readback is cleared by its existing
owner on drop; no plaintext key leaves the existing unwrap validator.

Pure fixtures exercise original-context refusal, unsupported platforms and exact
policy/candidate mismatch without credential calls. Root owns compilation and Native
qualification at the integrated source revision.

## Qdrant API selected-secret lease

`QdrantApiSecretStore<C>` implements the read-only `SecretStorePort` surface for the fixed
`secret-purpose:qdrant-api-key/v1` purpose. Root supplies the selected opaque reference,
installation ID/incarnation and the current user's independently observed SID-scope digest;
`QdrantApiSecretReference::for_owner` binds those facts before the platform read. The adapter derives
one exact Credential Manager target from that binding, reads it through the existing bounded
`read_provider_credential` owner, validates the versioned `ELQDRT01` record and selection digest,
then opens the protected payload through current-user DPAPI with Qdrant-only entropy that also binds
the exact nonzero record version. The target locator remains stable across versions, but changing the
unauthenticated header version changes DPAPI entropy and therefore fails closed. There is no fallback
to the former unversioned entropy format. The exact versioned `SecretReference` is passed to Core's
finite `SecretLease::issue_selected` lifecycle.
The issued expiry is the earlier of the configured lease TTL and the remaining original operation
allowance. The lease retains the issuer cancellation capability, request ID, budget reference and
relative deadline; each disclosure requires those exact request coordinates and a live issuer
deadline/cancellation state. A later context cannot extend or substitute that issuer authority.
There is no detached launch-evidence value: a historical copied tick cannot authorize process launch.
`QdrantApiSecretLease::with_secret` checks the issuer coordinates and the native monotonic TTL both
before and after the callback. Daemon composition must perform the effect that consumes the secret
inside this live callback and retain any spawned-child recovery custody if the callback refuses after
an effect attempt.

The store cannot create, rotate or delete records. A missing or malformed selected record returns a
content-free failure; it never searches other credentials, falls back to provider-pairing or
revision-DEK purposes, or creates a replacement. Plaintext leaves the returned private lease only
inside its bounded callback. This adapter does not establish artifact qualification, process
readiness or runtime activation; daemon composition still must use the actual Root custody and the
original operation context.

## Derived-artifact exact key use

`seal_derived_artifact` consumes the fresh, non-Clone C02
`CommittedDerivedArtifactSealUse`. `open_derived_artifact` requires C02's exact
reservation readback. Both compare the C07-owned descriptor, source coordinates,
all six canonical residency domains, selected Current, complete AAD, counter,
plaintext length and authenticated-block charge before native key access. The
selected Core binding supplies the existing opaque key reference, key version
and exact preparation class; a conflicting C07 key binding is rejected.

These calls reuse the existing private key window, configured TTL, exact
Credential Manager read/DPAPI unwrap, current-user recheck and original caller
accounting. Seal consumes the permit on every outcome and never refunds its
burned counter. The pure crypto owner returns ciphertext/tag or zeroizing
plaintext; C07 retains its descriptor/AAD/ELSDRV01 codec and independently checks
the returned bytes. This adapter has no credential creation, journal mutation,
source-history append or raw-key export surface.

The narrow dependency on C07 is metadata-only: it prevents reimplementing C07's
codec in the platform adapter. Root must enforce actual source/root/policy/purge
custody and supply the original context. Native qualification and ordinary
producer integration remain separate obligations.
