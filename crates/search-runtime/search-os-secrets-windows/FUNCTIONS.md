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
