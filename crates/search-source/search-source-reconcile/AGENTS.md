# Agent contract — search-source-reconcile

You own only `crates/search-source/search-source-reconcile/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S13.2, S14.1, S16.1-S16.2, P09.

## Mission

Turn watcher/USN hints and bounded inventories into truthful currentness, shadows and reconciliation work.

## Ownership

- watcher hint ingestion
- cursor continuity and gap state
- startup/resume/periodic reconciliation plans
- inventory diffs and source-head observations
- observation freshness classification

## Forbidden ownership

- treating watchers as complete source truth
- reading file bytes directly
- publishing index epochs
- claiming current workspace across a gap
- depending on the concrete redb adapter; durable state is reached through a vendor-neutral port

## Allowed dependencies

`search-contracts`, `search-domain`, `search-source-registry`, `search-source-identity`, `search-safe-reader`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `ingest_change_hint(hint, cursor) -> ReconcileSignal`
- `detect_observation_gap(previous, next) -> GapState`
- `plan_reconciliation(snapshot, budget) -> ReconcilePlan`
- `apply_inventory_diff(snapshot, observations) -> ReconcileOutcome`
- `preflight_current_workspace(view, freshness) -> Result<CurrentnessPermit, ReconcileError>`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `OBSERVATION_GAP`, `INDEX_GAP`, `SOURCE_HEAD_MISMATCH`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `watcher overflow creates OBSERVATION_GAP`
- `resume reconciliation restores continuity only after inventory proof`
- `current_workspace preflight denies unresolved gaps`
- `selected candidate live-head mismatch creates shadow/reconcile request`
- `deletion shadow blocks stale published membership`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W5 / P09**
- Soft `src/` target: **7,000 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.
