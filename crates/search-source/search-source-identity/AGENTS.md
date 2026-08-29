# Agent contract — search-source-identity

You own only `crates/search-source/search-source-identity/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S7.2.1, S7.3, S7.4, H3.3, P03.

## Mission

Derive stable source identity, retain path history and enforce single-writer namespace ownership and cutover.

## Ownership

- SourceIdentity derivation
- PathBinding history
- revision occurrence identity hooks
- SourceNamespaceOwnership state machine
- cutover receipt validation and fencing

## Forbidden ownership

- corpus/access policy inside SourceIdentity
- file content reads
- retrieval membership or ranking

## Allowed dependencies

`search-contracts`, `search-domain`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `derive_source_identity(observation) -> Result<SourceIdentity, IdentityError>`
- `reconcile_path_binding(identity, locator, revision) -> Result<PathBindingChange, IdentityError>`
- `prepare_cutover(state, command) -> Result<PreparedCutover, IdentityError>`
- `fence_old_owner(state, receipt) -> Result<OwnershipState, IdentityError>`
- `activate_new_owner(state, receipt) -> Result<OwnershipState, IdentityError>`
- `validate_cutover_receipt(receipt, expected) -> Result<(), IdentityError>`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `SOURCE_NAMESPACE_OWNERSHIP_CONFLICT`, `SOURCE_OWNER_CUTOVER_REQUIRED`, `IDENTITY_MAPPING_MISMATCH`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `rename and hardlink fixtures preserve identity/path distinctions`
- `A-B-A content creates three revision occurrences`
- `identity contains no membership/access fields`
- `concurrent dual writer is denied`
- `old owner is fenced before new owner activation`
- `mismatched generation/view/revision-set cutover receipt fails closed`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W2 / P03**
- Soft `src/` target: **6,500 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.
