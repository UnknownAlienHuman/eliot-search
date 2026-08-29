# Agent contract — search-safe-reader

You own only `crates/search-source/search-safe-reader/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S6.4, S15, S16.3-S16.6, H6.3, P03.

## Mission

Acquire exact source bytes without executing content, escaping admitted roots or mislabeling unstable files.

## Ownership

- final-handle root containment
- stable before/after metadata checks
- bounded retry and digest verification
- Git-object reads with hooks/prompts disabled
- encoding and byte-length observations

## Forbidden ownership

- parsing/materialization policy
- running hooks, macros, builds or remote resources
- following unadmitted reparse/symlink escapes
- retaining bytes durably

## Allowed dependencies

`search-contracts`, `search-domain`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `resolve_final_source(locator, root_policy) -> Result<ResolvedSource, ReadError>`
- `stable_read(resolved, budget) -> Result<StableRead, ReadError>`
- `read_git_object_no_execute(repo, oid, path) -> Result<StableRead, ReadError>`
- `verify_no_escape(resolved, admitted_roots) -> Result<(), ReadError>`
- `classify_source_admission(metadata, policy) -> AdmissionDecision`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `SOURCE_UNSTABLE`, `SOURCE_ESCAPE_DENIED`, `SOURCE_ADMISSION_DENIED`, `SOURCE_REVISION_UNAVAILABLE`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `symlink/reparse escape denied after final resolution`
- `cycles and nested roots handled deterministically`
- `file changing during read becomes SOURCE_UNSTABLE`
- `Git hooks and credential prompts cannot execute`
- `secret/system default exclusions do not leak matched content`
- `non-UTF8 and huge-file behavior is explicit`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W2 / P03**
- Soft `src/` target: **7,500 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.
