# Agent contract — search-publication

You own only `crates/search-index-qdrant/search-publication/`. Do not edit another package, the root
workspace, shared contracts or architecture. Missing fields use the contract-change process.

The Architecture 8.4 master is not required for ordinary work. This is the package slice.
Traceability only: S13, S36, H11, P07.

## Mission

Serialize projection commits, verify exact readback and linearize visibility only through guarded
control-journal commit.

## Ownership

- publication actor and state machine
- durable intent/receipt orchestration through `ControlJournalPort`
- exact new/old point mutation sequence through `SearchIndexPort`
- generation-guard CAS commit
- recovery, compensation and doctor-command domain
- committed retired-point manifests handed to `search-index-reclaimer`

## Forbidden ownership

- multiple active commit transactions
- reusing skipped epochs
- Qdrant alias as commit point
- broad payload closure on correctness paths
- staging a later epoch while an earlier intent is unresolved
- depending on concrete redb, Qdrant bridge or process-supervisor crates
- deleting retired points itself

## Allowed dependencies

`search-contracts`, `search-domain`, `search-projection-planner`, `search-point-identity`,
`search-epoch-pins`. Durable journal and index operations are injected through vendor-neutral ports.

## Required logical surface

- `PublicationCoordinator::submit(prepared, ports) -> Result<PublicationReceipt, PublicationError>`
- `PublicationCoordinator::recover(intent, ports) -> RecoveryDecision`
- `verify_exact_readback(manifest, readback) -> Result<(), PublicationError>`
- `commit_visible_epoch(guards, receipt, control) -> Result<ControlCommit, PublicationError>`
- `emit_retired_manifest(commit) -> RetiredPointManifest`
- `doctor_publication(command, ports) -> Result<DoctorReceipt, PublicationError>`

## Failure surface

Relevant reasons include `PUBLICATION_BLOCKED`, `PUBLICATION_READBACK_MISMATCH`,
`POINT_ID_COLLISION`, `CONTROL_COMMIT_REJECTED` and `PUBLICATION_RECOVERY_REQUIRED`.

## Test seams and exit evidence

- `every H11.5 failpoint kill/reopen matrix`
- `uncommitted epoch is never visible`
- `guard race failpoint blocks stale commit`
- `exact readback mismatch blocks publication`
- `skipped epoch is never reused`
- `abandon requires verified exclusion fence`
- `fake journal/index ports prove no concrete adapter dependency`
- `reclaimer receives only a committed exact retired manifest`

## Size and split guard

- Delivery wave: **W3 / P07**
- Soft `src/` target: **9,500 lines**
- Hard review threshold: **10,000 hand-written Rust lines**
- Commit and recovery remain one owner; split only on a proven dependency/process boundary.

## Definition of done

The state machine is linearizable and fault-tested through ports, never imports redb/Qdrant vendor
adapters and hands ordinary deletion to the reclaimer only after commit.
