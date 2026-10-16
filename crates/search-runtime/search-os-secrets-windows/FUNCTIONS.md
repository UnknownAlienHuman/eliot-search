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

This leaf performs no RNG, credential write/delete, lease, encryption, TTL/grace/ref
enforcement or whole-daemon activation. Protected readback is cleared by its existing
owner on drop; no plaintext key leaves the existing unwrap validator.

Pure fixtures exercise original-context refusal, unsupported platforms and exact
policy/candidate mismatch without credential calls. Root owns compilation and Native
qualification at the integrated source revision.

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
