# Agent contract — search-revision-store

You own only `crates/search-source/search-revision-store/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S6.3-S6.4, S7.5, S15, S28.4, H6, P04.

## Mission

Admit, retain and reopen immutable source revisions under complete residency identities.

## Ownership

- residency-key-derived CAS paths
- atomic temp/fsync/rename writes
- raw revision and manifest integrity
- retention leases and exact reopen
- copy/re-encrypt transition receipts

## Forbidden ownership

- query language or ranking
- global content-digest-only CAS namespace
- cross-domain co-residency, ciphertext or key reuse
- source identity or access authorization

## Allowed dependencies

`search-contracts`, `search-domain`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `derive_object_path(residency_key, kind, digest) -> CasPath`
- `admit_revision(stable_read, residency) -> Result<RevisionReceipt, CasError>`
- `reopen_exact(revision_ref) -> Result<VerifiedRevision, CasError>`
- `retain(revision_ref, cause) -> Result<RetentionLease, CasError>`
- `copy_or_reencrypt(source, target_residency) -> Result<TransitionReceipt, CasError>`
- `enumerate_mark_roots(snapshot) -> MarkRootSet`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `SOURCE_REVISION_UNAVAILABLE`, `RESIDENCY_DOMAIN_MISMATCH`, `CAS_INTEGRITY_MISMATCH`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `same bytes different residency keys produce different objects`
- `atomic-write crash leaves no admitted partial object`
- `reopen verifies residency digest, content digest and length`
- `cross-domain physical/ciphertext/key reuse denied`
- `visible epoch or durable handle keeps revision reachable`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W2 / P04**
- Soft `src/` target: **8,000 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.
