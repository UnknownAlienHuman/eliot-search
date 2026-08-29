# Agent contract — search-safe-reader

You own only `crates/search-source/search-safe-reader/`. Do not edit another package, the root
workspace, shared contracts or architecture. Missing fields use the contract-change process.

The Architecture 8.4 master is not required for ordinary work. This is the package slice.
Traceability only: S6.4, S15, S16.3-S16.5, H6.3, P03.

## Mission

Acquire exact source bytes without executing content, escaping an already admitted root or
mislabeling unstable data.

## Ownership

- final-handle root containment
- stable before/after metadata checks
- bounded retry, byte-length and digest verification
- Git-object reads with hooks, prompts, filters and network disabled
- encoding observations without changing the source-of-truth bytes

## Forbidden ownership

- source-admission policy or sensitivity decisions
- root registration, source identity or membership state
- parsing/materialization policy
- running hooks, macros, builds, filters or remote resources
- following unadmitted reparse/symlink escapes
- retaining bytes durably

## Allowed dependencies

`search-contracts`, `search-domain`. The caller supplies an admitted locator/root policy and any
admission receipt. This package must not import `search-source-admission` and re-evaluate policy.

## Required logical surface

- `resolve_final_source(locator, admitted_root) -> Result<ResolvedSource, ReadError>`
- `stable_read(resolved, budget) -> Result<StableRead, ReadError>`
- `read_git_object_no_execute(repo, oid, path) -> Result<StableRead, ReadError>`
- `verify_no_escape(resolved, admitted_roots) -> Result<(), ReadError>`
- `verify_stability(before, after, bytes) -> Result<StableReadReceipt, ReadError>`

## Failure surface

Relevant reasons include `SOURCE_UNSTABLE`, `SOURCE_ESCAPE_DENIED`,
`SOURCE_REVISION_UNAVAILABLE`, `SOURCE_TOO_LARGE` and `SOURCE_ENCODING_UNSUPPORTED`.

## Test seams and exit evidence

- `symlink/reparse escape denied after final resolution`
- `cycles nested roots hardlinks and path replacement handled deterministically`
- `file changing during read becomes SOURCE_UNSTABLE`
- `Git hooks filters credential prompts and network cannot execute`
- `non-UTF8 and huge-file behavior is explicit`
- `reader cannot widen or invent source-admission policy`

## Size and split guard

- Delivery wave: **W2 / P03**
- Soft `src/` target: **6,500 lines**
- Hard review threshold: **10,000 hand-written Rust lines**
- Split filesystem and Git backends only when native dependencies or measured size create a real
  replacement/test boundary.

## Definition of done

The package has a vendor-neutral public contract, deterministic containment/stability tests, explicit
degradation behavior and no policy or durable-storage ownership.
