# `search-safe-reader` implementation packet

**Path:** `crates/search-source/search-safe-reader`  
**Capability:** C06  
**Delivery:** W2 / P03  
**Gate:** BLOCKED until W1 control receipt and W0 contracts are accepted  
**Trace:** S6.4, S15, S16.3-S16.5, H6.3, P03  
**Direct public handoffs:** `search-contracts`, `search-domain`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust spelling.

## Mission

Acquire exact source bytes without executing content, escaping an already admitted root or mislabeling unstable data.

## Owns

- final-handle resolution and admitted-root containment
- pre/post identity and metadata observations
- bounded retries, byte-length and digest verification
- no-execute filesystem and Git-object acquisition
- encoding observations without changing source-truth bytes

## Must not own

- source-admission or sensitivity policy decisions
- root registration, source identity or membership state
- parsing/materialization or durable byte retention
- executing hooks, filters, macros, build scripts, credential prompts or remote resources
- following unadmitted reparse/symlink escapes

## Logical primitives

- `SafeReadRequest`, `RootTraversalPolicy`, `ResolvedSourceHandle`, `FileObservation`, `StableReadAttempt`, `StabilityReceipt`, `ReadBudget`

## Logical operations

1. `resolve_final_handle(locator, admitted_root) -> Result<ResolvedSourceHandle, ReadError>`
2. `read_stably(handle, budget) -> Result<StableBytes, ReadError>`
3. `read_git_object_no_execute(repo, oid, path, budget) -> Result<StableBytes, ReadError>`
4. `verify_pre_post(before, after, bytes) -> Result<StabilityReceipt, ReadError>`
5. `verify_no_escape(handle, admitted_roots) -> Result<(), ReadError>`

## Required invariants

- root policy is applied after final-handle resolution
- reparse/symlink escape is denied unless the target root is independently admitted
- unstable files retry only within budget then fail explicitly
- Git hooks, filters, prompts and network are disabled
- source-admission cannot be widened or invented by the reader
- secret-bearing bytes and absolute paths are absent from default diagnostics

## Typed failure surface

- `SOURCE_UNSTABLE`
- `PATH_ESCAPES_ADMITTED_ROOT`
- `REPARSE_CYCLE`
- `NO_EXECUTE_POLICY_DENIED`
- `SOURCE_TOO_LARGE`
- `SOURCE_ENCODING_UNSUPPORTED`

## Exit tests / evidence

- `symlink_and_reparse_escape_denied`
- `cycle_and_path_replacement_detected`
- `file_changes_during_read_becomes_source_unstable`
- `git_hooks_filters_prompts_and_network_never_execute`
- `huge_file_budget_enforced`
- `reader_cannot_widen_admission_policy`
- `non_utf8_behavior_explicit`

## Suggested internal modules

```text
search-safe-reader/src/
  resolve.rs
  stable_read.rs
  filesystem.rs
  git_object.rs
  metadata.rs
  digest.rs
  encoding.rs
  error.rs
```

## Size / split

- Initial `src/` target: **≤ 6,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Split filesystem and Git backends only when native dependencies or measured size create a real boundary.
