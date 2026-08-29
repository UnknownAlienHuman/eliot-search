# Agent contract — search-publication

You own only `crates/search-index-qdrant/search-publication/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S13, S36, H11, P07.

## Mission

Serialize projection commits, verify exact readback and linearize visibility only through guarded control-journal commit.

## Ownership

- publication actor and state machine
- durable intents and receipts orchestration
- exact new/old point mutation sequence
- generation guard CAS commit
- recovery, compensation and doctor command domain

## Forbidden ownership

- multiple active commit transactions
- reusing skipped epochs
- Qdrant alias as commit point
- broad payload closure on correctness paths
- staging later epoch while earlier is unresolved
- depending on the concrete redb adapter; durable state is reached through a vendor-neutral port

## Allowed dependencies

`search-contracts`, `search-domain`, `search-projection-planner`, `search-point-identity`, `search-qdrant-bridge`, `search-epoch-pins`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `PublicationCoordinator::submit(prepared) -> Result<PublicationReceipt, PublicationError>`
- `PublicationCoordinator::recover(intent) -> RecoveryDecision`
- `verify_exact_readback(manifest, readback) -> Result<(), PublicationError>`
- `commit_visible_epoch(guards, receipt) -> Result<ControlCommit, PublicationError>`
- `doctor_publication(command) -> Result<DoctorReceipt, PublicationError>`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `PUBLICATION_BLOCKED`, `PUBLICATION_READBACK_MISMATCH`, `POINT_ID_COLLISION`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `every H11.5 failpoint kill/reopen matrix`
- `uncommitted epoch is never visible`
- `guard race failpoint blocks stale commit`
- `exact readback mismatch blocks publication`
- `skipped epoch is never reused`
- `abandon requires verified exclusion fence`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W3 / P07**
- Soft `src/` target: **9,500 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.
