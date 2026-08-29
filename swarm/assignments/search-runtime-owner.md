# `search-runtime-owner` implementation packet

**Path:** `crates/search-runtime/search-runtime-owner`  
**Capability:** C01  
**Delivery:** W1 / P01  
**Gate:** BLOCKED until W0 receipt is accepted  
**Trace:** S7.1, S27.1-S27.4, S33, H8.4, P01  
**Direct public handoffs:** `search-contracts`, `search-domain`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Guarantee that exactly one process incarnation owns one data root and expose a fenced lifecycle to the daemon.

## Owns

- data-root lease and owner record
- owner epoch/incarnation fencing
- standalone versus managed mode fence
- drain, orderly release and abandoned-owner recovery

## Must not own

- retrieval or source semantics
- Qdrant schema/query behavior
- control-journal table ownership
- treating a responding PID/port as proof of ownership

## Logical primitives

- OwnerMode, OwnerLifecycleState, OwnerRecord, OwnerLease, OwnerGuard, DrainToken, ShutdownReceipt, RecoveryDecision

## Logical operations

1. `acquire_data_root(path, mode, installation) -> Result<OwnerGuard, OwnerError>`
2. `verify_owner_record(record, process_identity) -> Result<(), OwnerError>`
3. `recover_abandoned_owner(record, observed_processes) -> RecoveryDecision`
4. `OwnerGuard::begin_drain() -> DrainToken`
5. `OwnerGuard::release_cleanly(token) -> Result<ShutdownReceipt, OwnerError>`

## Required invariants

- one live owner per data root
- managed and standalone modes never co-own a root
- owner epoch never regresses or aliases an abandoned incarnation
- mode change requires drain, fence and restart
- ownership proof includes process/executable/data-root identity, not only a lock file

## Typed failure surface

- `DATA_ROOT_ALREADY_OWNED`
- `OWNER_EPOCH_MISMATCH`
- `OWNER_IDENTITY_AMBIGUOUS`
- `MODE_TRANSITION_REQUIRES_RESTART`
- `OWNER_RECOVERY_QUARANTINED`

## Exit tests / evidence

- `second_owner_denied`
- `crash_reopen_fences_or_advances_owner_epoch`
- `mode_coownership_rejected`
- `stale_lock_without_matching_process_recovers_safely`
- `orderly_shutdown_releases_owner`

## Suggested internal modules

```text
search-runtime-owner/src/
  record.rs
  lease.rs
  recovery.rs
  drain.rs
  platform.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 4,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Keep platform-specific locking behind a private adapter. Split only if another platform needs an independently tested implementation package.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.
