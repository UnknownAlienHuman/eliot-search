# `search-source-identity` implementation packet

**Path:** `crates/search-source/search-source-identity`  
**Capability:** C04  
**Delivery:** W2 / P03  
**Gate:** BLOCKED until W1 control receipt and W0 contracts are accepted  
**Trace:** S7.2-S7.4, S16.4, P03  
**Direct public handoffs:** `search-contracts`, `search-domain`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Resolve durable physical/logical source identity and path history without attaching corpus or access policy.

## Owns

- identity observations and canonical identity keys
- PathBinding creation, rename and closure transitions
- hard-link, case and normalization handling
- repository/worktree lineage identity helpers

## Must not own

- corpus membership or access decisions
- reading file contents or deciding revision stability
- treating a path as source identity
- silently merging nested repositories or submodules

## Logical primitives

- IdentityObservation, StableIdentityComponents, CanonicalPathKey, PathBindingState, PathBindingTransition, RepositoryLineageObservation, WorkspaceIdentityObservation

## Logical operations

1. `resolve_identity(observation, prior_candidates) -> IdentityResolution`
2. `derive_canonical_path_key(path, filesystem_profile) -> CanonicalPathKey`
3. `transition_path_binding(state, event) -> Result<PathBindingState, IdentityError>`
4. `relate_hardlink_bindings(observations) -> IdentityGrouping`
5. `classify_repository_lineage(observation) -> RepositoryLineageDecision`

## Required invariants

- paths are locators, never identity
- SourceIdentity has no membership/policy fields
- hard links may have multiple active PathBindings to one identity
- rename closes/opens bindings without inventing a new source when identity is stable
- nested repositories and submodules remain distinct workspaces

## Typed failure surface

- `SOURCE_IDENTITY_AMBIGUOUS`
- `PATH_BINDING_CONFLICT`
- `PATH_ESCAPES_ADMITTED_ROOT`
- `FILESYSTEM_IDENTITY_UNSUPPORTED`
- `LINEAGE_IDENTITY_AMBIGUOUS`

## Exit tests / evidence

- `rename_preserves_identity`
- `hardlink_multiple_bindings_one_identity`
- `case_and_unicode_lookup_key_roundtrip`
- `nested_repository_not_collapsed`
- `path_reuse_does_not_reuse_closed_identity_without_proof`

## Suggested internal modules

```text
search-source-identity/src/
  physical.rs
  path_key.rs
  binding.rs
  lineage.rs
  workspace.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 6,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Split OS-specific identity acquisition only when its platform dependency becomes independently replaceable; keep pure identity decisions portable.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.
