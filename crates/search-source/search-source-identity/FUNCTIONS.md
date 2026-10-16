# Function contract — `search-source-identity`

The explicit `ntfs-directory/v1` codec retains full volume/index/creation values
without a path, Root UUID or policy in the physical tuple. It encodes supplied
observations only; actual FD custody and current permission checks remain Native
producer obligations. Shared bounded tuple/binary helpers are value construction,
not an additional canonical encoder or native identity/grant issuer.

The additive `ntfs_source_profile_v1()` returns a fixed **descriptor**, not Native
qualification. Its revision is 1: pre-normalized insensitive lookup, preserved
Unicode scalar values, required volume/file identities, stable physical hard-link
identity and rejected reparses. Its schema digest is shared canonical CBOR/BLAKE3
over the complete descriptor: revision, canonical encoding profile, three digest
domains, six behavior fields, and the labeled Native ABI/name/case/encoding/tuple
contracts in `ntfs_source::NATIVE_CONTRACT`. Any change requires a new profile.

`ntfs_volume_identity_v1(volume_guid, volume_serial)` hashes the exact tuple
`[1, "windows", "ntfs", volume_guid, volume_serial]` in
`eliot/cbor/source-identity/ntfs-volume/v1`. The GUID is exactly the 49 ASCII bytes
of `\\?\Volume{xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx}\`, with lowercase hex and no
suffix. `ntfs_file_identity_v1(file_index, creation_time)` hashes
`[1, "windows", "ntfs", file_index, creation_time]` in the separate
`eliot/cbor/source-identity/ntfs-file/v1` domain. Full unsigned index/creation
FILETIME values are retained; creation protects against index reuse. Pair the
file digest with the volume digest and use `generation: None`, not a fabricated
counter. Existing `ntfs-directory/v1` bytes and behavior are unchanged.

All three functions return `Result<_, IdentityError>` and perform no Native I/O.
The Native caller must obtain actual held-FD volume/file/creation values, prove
current Root/FD custody, reject reparses, measure Root and every Root-relative
naming directory as case-insensitive, and obtain native normalized stored long
names with strict UTF-16 decoding before attesting. Paths remain locators; no
ASCII folding, Unicode normalization, drive-letter mapping, fixture default or
schema digest substitutes for these proofs. Pure fixtures cover descriptor and
digest semantics only, not Native qualification.

**Status:** W2/P03 logical contract; no Windows filesystem/repository identity evidence exists.

This package owns pure source/path/repository/workspace identity decisions over bounded observations. It
performs no file read, content hashing, registry mutation, admission/access decision or source retention.
Paths are locators and history records, never source identity by themselves.

## Global rules

- public source identity contains physical/logical identity and lineage only—no corpus, membership,
  access, admission or disclosure policy;
- every identity decision records observation/profile/schema identities and material ambiguity;
- case/Unicode/path normalization is a lookup profile, not proof that two physical objects are equal;
- multiple active hard-link path bindings may point to one source identity;
- rename/path reuse is represented by explicit binding transitions;
- nested repositories, worktrees and submodules remain explicit unless exact lineage evidence relates
  them without collapsing workspace boundaries;
- equal canonical observations produce deterministic decisions and IDs.

## Observation validation

### `validate_filesystem_profile(profile) -> Result<FilesystemIdentityProfile, IdentityError>`

Requires exact platform/filesystem behavior for case sensitivity, Unicode normalization, volume/file IDs,
link/reparse semantics and unsupported/unavailable fields. Implicit host defaults are rejected.

### `validate_identity_observation(input, profile, limits) -> Result<IdentityObservation, IdentityError>`

Requires bounded root identity, final-handle path key, physical stable components where available,
metadata generation/time/size hints, prior binding candidates and explicit confidence/unavailable fields.
It accepts no source body or policy field.

### `derive_canonical_path_key(path_observation, profile) -> Result<CanonicalPathKey, IdentityError>`

Builds a versioned lookup key with explicit root/filesystem/case/Unicode behavior. It never returns a
`SourceIdentity` or merges bindings solely from text equality.

## Source identity resolution

### `resolve_identity(observation, prior_candidates, policy) -> Result<IdentityResolution, IdentityError>`

Returns exactly:

```text
MATCH_EXISTING(source_id, evidence)
CREATE_NEW(identity_draft, evidence)
AMBIGUOUS(candidates, missing_evidence)
UNSUPPORTED(reason)
```

Stable physical components and explicit lineage evidence dominate path similarity. Path reuse after a
closed binding cannot resurrect the old identity without accepted evidence.

Cancellation/budget exhaustion yields no successful match/create result.

### `derive_source_identity(validated_resolution, namespace) -> Result<SourceIdentity, IdentityError>`

Domain-separates canonical stable identity components plus namespace/schema. It rejects ambiguous/path-
only resolution and default namespace/identity fields.

### `compare_identity(expected, observed) -> IdentityMatchDecision`

Returns exact match, material mismatch, insufficient evidence or unsupported profile. It never downgrades
missing load-bearing components to match.

## Path binding state

### `open_path_binding(source, locator, root, observation, owner_generation, operation) -> Result<PathBinding, IdentityError>`

Creates one active binding only when source resolution/root/owner generation match and no conflicting
active binding occupies the canonical path key. The operation is pure state transition; persistence is a
registry/control-port concern.

### `transition_path_binding(current, event, observations) -> Result<PathBindingTransition, IdentityError>`

Closed events include:

```text
RENAME
MOVE_WITHIN_ROOT
MOVE_TO_OTHER_ADMITTED_ROOT
HARDLINK_ADDED
HARDLINK_REMOVED
PATH_REPLACED
SOURCE_REMOVED
ROOT_UNBOUND
```

Rename/move closes the prior locator interval and opens a new one without inventing a source when exact
identity persists. Path replacement closes the old binding and requires fresh identity resolution.

### `relate_hardlink_bindings(observations, profile) -> Result<IdentityGrouping, IdentityError>`

Groups only bindings with exact accepted physical identity evidence. Text-normalized path or content
similarity alone cannot establish hard-link identity.

### `validate_binding_history(history) -> Result<PathHistoryReceipt, IdentityError>`

Proves non-overlapping active intervals per canonical path key, monotonic revisions and explicit close/
open reasons. It preserves aliases/hard links rather than forcing one canonical display path.

## Repository and workspace lineage

### `validate_repository_observation(input, limits) -> Result<RepositoryLineageObservation, IdentityError>`

Requires exact local repository/worktree/submodule identity observations, root boundaries and explicit
missing fields. It performs no Git command/network operation.

### `classify_repository_lineage(observation, prior) -> Result<RepositoryLineageDecision, IdentityError>`

Returns independent lineage, exact same lineage/worktree relation, fork/mirror/proven-copy relation,
submodule/nested boundary or ambiguous. A remote URL/name alone is insufficient.

### `derive_workspace_identity(repository, worktree, branch_or_index_state) -> Result<WorkspaceIdentity, IdentityError>`

Builds one explicit workspace identity/version fence. Nested repositories and submodules are not merged
into the parent workspace. Branch/index changes create a new workspace-view revision through the
registry owner; this operation only derives identity inputs.

## Batch and redaction operations

### `resolve_batch(observations, prior_index, limits, budget, cancel) -> Result<IdentityBatchDecision, IdentityError>`

Canonicalizes finite inputs, detects duplicate/conflicting path/physical observations and returns one
explicit outcome per item. Cancellation cannot silently create/match unprocessed identities.

### `redacted_identity_view(identity_or_binding, disclosure) -> RedactedIdentityView`

Returns opaque IDs, root/location class, binding state/revision and reason/confidence classes. It excludes
unrestricted paths, source content and foreign workspace details.

## Cancellation, deadline and retry

Decisions are pure and retry-safe. Finite budgets/cancellation are checked during large candidate/batch/
history/lineage operations. There is no durable mutation or unknown commit outcome. Registry persistence
uses its own stable mutation identity and revalidates these decisions.

## Typed failures

- `FILESYSTEM_PROFILE_UNSUPPORTED`
- `IDENTITY_OBSERVATION_INVALID`
- `SOURCE_IDENTITY_AMBIGUOUS`
- `SOURCE_IDENTITY_INSUFFICIENT_EVIDENCE`
- `SOURCE_IDENTITY_CONFLICT`
- `PATH_BINDING_CONFLICT`
- `PATH_BINDING_HISTORY_INVALID`
- `PATH_ESCAPES_ADMITTED_ROOT`
- `PATH_REUSE_REQUIRES_NEW_RESOLUTION`
- `HARDLINK_IDENTITY_UNPROVED`
- `LINEAGE_IDENTITY_AMBIGUOUS`
- `REPOSITORY_BOUNDARY_CONFLICT`
- `WORKSPACE_IDENTITY_INVALID`
- `IDENTITY_BUDGET_EXHAUSTED`
- `IDENTITY_CANCELLED`

## Required tests / qualification evidence

- path-only observation never creates/matches durable source identity;
- rename/move preserves identity with exact physical evidence;
- path reuse after replacement does not reuse the closed source;
- multiple hard links produce multiple bindings to one identity;
- case-sensitive/insensitive and Unicode-normalization lookup goldens;
- volume/file-ID unavailable/changed/reused/ambiguous matrices;
- binding history interval/monotonicity property tests;
- nested repository and submodule never collapse into parent workspace;
- worktree/same-lineage/fork/mirror/proven-copy/ambiguous fixtures;
- branch/index state creates distinct workspace identity inputs;
- batch duplicates/conflicts/cancellation account every item;
- SourceIdentity public schema contains no policy/membership/access field;
- no file/Git/network/content I/O dependency;
- redacted diagnostics exclude path/content/foreign workspace leakage.
