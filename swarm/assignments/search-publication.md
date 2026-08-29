# `search-publication` implementation packet

**Path:** `crates/search-index-qdrant/search-publication`  
**Capability:** C16  
**Delivery:** W3 / P07  
**Gate:** BLOCKED until projection/point contracts and journal/index port handoffs are accepted  
**Trace:** S13, S36, H11, P07  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-projection-planner`, `search-point-identity`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust spelling.

## Mission

Serialize projection commits, verify exact readback and linearize visibility only through guarded control-journal commit.

## Owns

- publication actor and state machine
- durable intent/receipt orchestration through `ControlJournalPort`
- exact new/old point mutation sequence through `SearchIndexPort`
- generation-guard CAS commit
- recovery, compensation and doctor-command domain
- committed `RetiredPointManifest` emission for the reclaimer

## Must not own

- multiple active commit transactions or skipped-epoch reuse
- Qdrant alias as commit point
- broad payload closure on correctness paths
- staging a later epoch while an earlier intent is unresolved
- concrete redb/Qdrant/process adapter dependencies
- physical retired-point deletion

## Logical primitives

- `PreparedPublication`, `PublicationIntent`, `PublicationState`, `PublicationGuardSet`, `PublicationReceipt`, `RetiredPointManifest`, `PublicationRecoveryDecision`

## Logical operations

1. `submit(prepared, ports) -> Result<PublicationReceipt, PublicationError>`
2. `recover(intent, ports) -> PublicationRecoveryDecision`
3. `verify_exact_readback(manifest, readback) -> Result<(), PublicationError>`
4. `commit_visible_epoch(guards, receipt, control) -> Result<ControlCommit, PublicationError>`
5. `emit_retired_manifest(commit) -> RetiredPointManifest`
6. `doctor(command, ports) -> Result<DoctorReceipt, PublicationError>`

## Required invariants

- one global active commit transaction
- an uncommitted epoch is never observable and no epoch is reused
- every external mutation is acknowledged and exactly read back
- owner/source/membership/access/shadow/purge guards are checked inside the control commit
- the reclaimer receives only a committed exact retired manifest
- broad-filter closure/deletion is unavailable on correctness paths

## Typed failure surface

- `PUBLICATION_BLOCKED`
- `PUBLICATION_READBACK_MISMATCH`
- `POINT_ID_COLLISION`
- `CONTROL_COMMIT_REJECTED`
- `PUBLICATION_RECOVERY_REQUIRED`

## Exit tests / evidence

- `every_H11_failpoint_kill_reopen_matrix`
- `uncommitted_epoch_never_visible`
- `guard_race_blocks_stale_commit`
- `exact_readback_mismatch_blocks_commit`
- `skipped_epoch_never_reused`
- `abandon_requires_verified_exclusion_fence`
- `fake_journal_and_index_ports_prove_adapter_independence`
- `retired_manifest_emitted_only_after_commit`

## Suggested internal modules

```text
search-publication/src/
  actor.rs
  state.rs
  intent.rs
  mutate.rs
  readback.rs
  commit.rs
  recovery.rs
  retired.rs
  doctor.rs
  error.rs
```

## Size / split

- Initial `src/` target: **≤ 7,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Commit and recovery remain one state-machine owner; physical reclaim stays separate.
