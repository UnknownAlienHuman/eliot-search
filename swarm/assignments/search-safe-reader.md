# `search-safe-reader` implementation packet

**Path:** `crates/search-source/search-safe-reader`  
**Capability:** C06  
**Delivery:** W2 / P03  
**Gate:** BLOCKED until W1 control receipt and W0 contracts are accepted  
**Trace:** S16.3-S16.6, H6.3, P03  
**Direct public handoffs:** `search-contracts`, `search-domain`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Acquire coherent source bytes through stable no-execute reads and final-handle root policy checks.

## Owns

- final-handle resolution and admitted-root containment
- pre/post identity and metadata observations
- bounded retries and content digest verification
- no-execute acquisition policy and stability receipts

## Must not own

- executing hooks, macros, build scripts, credential prompts or remote resources
- extracting archive members in the baseline
- parsing/materializing content
- logging secret-bearing bytes or absolute paths by default

## Logical primitives

- SafeReadRequest, RootTraversalPolicy, ResolvedSourceHandle, FileObservation, StableReadAttempt, StabilityReceipt, NoExecuteDecision, ReadBudget

## Logical operations

1. `resolve_final_handle(locator, root_policy) -> Result<ResolvedSourceHandle, ReadError>`
2. `evaluate_no_execute_policy(handle, source_class) -> NoExecuteDecision`
3. `read_stably(handle, budget) -> Result<StableBytes, ReadError>`
4. `verify_pre_post_observations(before, after, bytes) -> Result<StabilityReceipt, ReadError>`
5. `classify_sensitive_source(metadata, detectors) -> SensitivityObservation`

## Required invariants

- root policy is applied after final-handle resolution
- reparse/symlink escape is denied unless target root is independently admitted
- unstable files retry only within budget then fail explicitly
- read operation does not execute content or repository automation
- secret detectors never copy detected secret material into logs/index payload

## Typed failure surface

- `SOURCE_UNSTABLE`
- `PATH_ESCAPES_ADMITTED_ROOT`
- `REPARSE_CYCLE`
- `NO_EXECUTE_POLICY_DENIED`
- `SOURCE_TOO_LARGE`
- `SOURCE_SENSITIVE_NOT_ADMITTED`

## Exit tests / evidence

- `symlink_and_reparse_escape_denied`
- `cycle_detected`
- `file_changes_during_read_becomes_source_unstable`
- `git_hook_and_macro_never_executed`
- `huge_file_budget_enforced`
- `sensitive_fixture_has_content_minimized_diagnostics`

## Suggested internal modules

```text
search-safe-reader/src/
  resolve.rs
  policy.rs
  stable_read.rs
  metadata.rs
  digest.rs
  sensitivity.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 7,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Archive/document providers are separate optional materializers. Platform handle acquisition may split only if independently replaceable.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.
