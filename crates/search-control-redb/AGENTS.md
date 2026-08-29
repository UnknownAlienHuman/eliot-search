# Agent contract — search-control-redb

You own only `crates/search-control-redb/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S6.2, S13, S14.1, S28, H5, P02.

## Mission

Persist only bounded technical control state and publish immutable snapshots for read-only hot paths.

## Ownership

- journal schema and migrations
- publication intents/receipts and route metadata
- source/control references, cursors and fences
- atomic Arc<ControlSnapshot> publication
- corruption quarantine and write counters

## Forbidden ownership

- source bodies or extracted text
- postings, vectors or term statistics
- ranked candidate/query history storage
- reverse-engineering currentness from orphaned Qdrant data

## Allowed dependencies

`search-contracts`, `search-domain`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `ControlJournal::open(config) -> Result<Journal, JournalError>`
- `Journal::migrate(target_schema) -> Result<MigrationReceipt, JournalError>`
- `Journal::read_snapshot() -> Arc<ControlSnapshot>`
- `Journal::transaction(command) -> Result<ControlCommit, JournalError>`
- `Journal::quarantine(reason) -> Result<QuarantineReceipt, JournalError>`
- `Journal::write_counters() -> JournalWriteCounters`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `CONTROL_STORE_CORRUPT`, `RESTORE_PENDING_REVALIDATION`, `CONTROL_ROUTE_MISMATCH`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `required_table_set_matches H5.1`
- `migration fixtures are deterministic and idempotent`
- `power_loss_reopen_preserves committed control state only`
- `hot_query_does_not_mutate_redb after 10,000 admissions`
- `mismatched incarnation or collection route quarantines`
- `forbidden corpus payload cannot be serialized into journal records`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W1 / P02**
- Soft `src/` target: **8,500 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.
