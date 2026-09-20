# PR #193: source-root currentness ownership

Date: 2026-09-19
Tracking: issue #189 / PR #193 / T02 source-root ownership phase

## Ownership move

The daemon source-root catalog previously owned both concrete filesystem
observation and the complete pure currentness state machine:

- `SourceRootState`;
- watcher hint kinds, sequencing, capacity and overflow;
- observation-gap derivation;
- reconciliation generations;
- sync-proof invalidation and acknowledgement;
- source/workspace currentness projection;
- update-outcome-unknown fencing.

`search-source-registry` now owns those content-free semantics through
`SourceRootCurrentness`. The state vector is position-aligned with
caller-owned configured locators, but no locator or source content enters the
package.

The package owns:

- the closed qualified-observation vocabulary;
- the 64-hint bound;
- the 40-gap response bound;
- exact watcher sequence/overflow behavior;
- complete observation reconciliation;
- insert/remove/observe transitions aligned to registration mutations;
- generation advancement and sync-proof invalidation;
- update-outcome-unknown fail-closed state;
- explicit gap and truth snapshots.

A mismatched full observation count fails the owner closed as
`UpdateOutcomeUnknown`; it cannot silently realign states with different
locators.

## Daemon composition retained

`eliot-searchd::source_roots` retains:

- configured `PathBuf` locators;
- platform path validation, canonicalization and overlap policy;
- symlink/reparse handling;
- filesystem probing that produces one `SourceRootState` per locator;
- durable `source-roots.v1` publication and recovery;
- watcher adapter integration;
- translation of package mutation failures to existing `SOURCE_ROOT_*`
  reasons.

`SourceRootCatalog` now contains one package-owned `SourceRootCurrentness`
instead of its own `needs_reopen`, hint queue, overflow flag, generation and
last-synced fields. Root registration is published first; only then is the
aligned pure state inserted or removed. Any impossible post-publication
alignment failure is fenced as `SOURCE_ROOT_UPDATE_OUTCOME_UNKNOWN`.

## Dependency boundary

No dependency on W5 `search-source-reconcile` was added. The baseline daemon
already depends on W2 `search-source-registry`, whose contract owns roots and
coherent source/workspace views. `search-source-reconcile` remains optional and
continues to own later live reconciliation orchestration rather than this
legacy compatibility state.

## Preserved behavior

- watcher hints remain dirty markers only;
- watcher overflow creates an explicit gap and never proves availability;
- exact reconciliation consumes hints/overflow and advances the generation;
- unchanged exact observations without hints do not advance the generation;
- any state, registration-set, hint or overflow change invalidates sync proof;
- missing, non-directory, unsafe and unverifiable roots remain explicit gaps;
- zero configured roots are never source/workspace current;
- unresolved durable registration outcome reports one update-unknown gap and
  zero available roots;
- public daemon-local type names and field access remain available through
  package re-exports;
- persisted formats, command responses, dependencies, `Cargo.lock`, workflows,
  gate and launch state are unchanged.

## Regression coverage

Package tests cover:

- generation-bound reconciliation and sync proof;
- bounded watcher hints and overflow clearing;
- aligned insert/remove transitions;
- state-count mismatch fail-closed behavior;
- empty-registration currentness;
- stable spellings and bounds.

Daemon process tests continue to cover real filesystem swaps, missing roots,
non-directory replacement, watcher overflow, active-set mutation and sync-proof
invalidation. Ownership tests reject local daemon copies of the currentness
types/fields and reject filesystem/path/process/Qdrant ownership in the package
module.

## Required execution

```text
cargo test --locked -p search-source-registry root_currentness
cargo test --locked -p eliot-searchd --test source_roots_module_ownership
cargo test --locked -p eliot-searchd source_roots::kernel::tests
cargo check --locked -p search-source-registry -p eliot-searchd \
  --all-targets --all-features
cargo fmt --all -- --check
cargo clippy --locked -p search-source-registry -p eliot-searchd \
  --all-targets --all-features -- -D warnings
```

Execution status in the current authoring environment: **NOT_RUN** (`cargo`,
`rustc`, and `rustfmt` are absent). No native qualification, T02 acceptance, or
independent review is claimed.
