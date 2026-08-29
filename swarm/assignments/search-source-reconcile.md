# `search-source-reconcile` implementation packet

**Path:** `crates/search-source/search-source-reconcile`  
**Capability:** C05  
**Delivery:** W5 / P09  
**Gate:** BLOCKED until W4 query baseline and W2 source receipts are accepted  
**Trace:** S16.1-S16.2, S18, P09  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-source-registry`, `search-source-identity`, `search-safe-reader`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Turn watcher/USN hints and bounded inventories into truthful observation continuity and confirmed source change sets.

## Owns

- watcher cursor/gap state
- startup, resume, periodic and explicit reconcile planning
- bounded inventory comparison and confirmed change sets
- observation freshness classification and current-workspace preflight

## Must not own

- treating watcher events as source truth
- publishing index points
- persisting unsaved editor bytes
- claiming currentness across an unresolved gap

## Logical primitives

- ObservationCursor, ObservationGap, WatchHint, InventoryEntry, InventorySnapshot, ReconcilePlan, ConfirmedChangeSet, ObservationFreshness, CurrentWorkspacePreflight

## Logical operations

1. `ingest_watch_hint(state, hint) -> ReconcileState`
2. `detect_cursor_gap(previous, next) -> Option<ObservationGap>`
3. `plan_reconcile(reason, budget, roots) -> ReconcilePlan`
4. `compare_inventory(prior, observed) -> ConfirmedChangeSet`
5. `classify_freshness(state, now) -> ObservationFreshness`
6. `preflight_current_workspace(scope, state, budget) -> PreflightDecision`

## Required invariants

- watchers accelerate but never prove completeness
- overflow/resume creates a gap until bounded reconciliation closes it
- current_confirmed requires continuous relevant observation cursor
- live head mismatch shadows/drops candidate and schedules reconciliation
- historical frozen views may remain queryable from retained bytes during a live gap

## Typed failure surface

- `OBSERVATION_GAP`
- `RECONCILIATION_REQUIRED`
- `RECONCILIATION_BUDGET_EXHAUSTED`
- `SOURCE_HEAD_DRIFT`
- `INVENTORY_INCOMPLETE`

## Exit tests / evidence

- `watcher_overflow_then_reconcile`
- `resume_gap_blocks_current_claim`
- `startup_inventory_detects_missed_delete`
- `live_head_mismatch_emits_shadow_request`
- `historical_view_remains_truthfully_available`
- `bounded_sweep_never_reports_false_complete`

## Suggested internal modules

```text
search-source-reconcile/src/
  cursor.rs
  watch.rs
  inventory.rs
  plan.rs
  diff.rs
  freshness.rs
  preflight.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 7,000 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- USN/watcher platform acquisition may be an adapter module; continuity and inventory truth remain one capability.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.
