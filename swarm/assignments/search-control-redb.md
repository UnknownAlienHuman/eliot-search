# `search-control-redb` implementation packet

**Path:** `crates/search-control-redb`  
**Capability:** C02  
**Delivery:** W1 / P02  
**Gate:** BLOCKED until runtime owner shell and W0 contracts are accepted  
**Trace:** S6.2, S13-S14, S28, H5, P02  
**Direct public handoffs:** `search-contracts`, `search-domain`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Implement the bounded durable control journal and immutable control-snapshot publication; never become a search database.

## Owns

- redb schema and migrations for bounded technical control state
- atomic control transactions and idempotency rows for mutations
- publication/source/route/fence/cursor/tombstone records
- immutable ControlSnapshot reconstruction and publication receipts

## Must not own

- source bodies, extracted text, postings, vectors or ranked candidate sets
- ordinary query history or durable leases for hot reads
- reverse-engineering currentness from orphaned Qdrant points
- vendor types crossing the package port

## Logical primitives

- JournalIdentity, SchemaVersion, MigrationStep, ControlTransaction, ControlSnapshot, JournalOpenMode, QuarantineReason, SnapshotPublishReceipt

## Logical operations

1. `open_or_create(path, expected_identity) -> Result<Journal, JournalError>`
2. `migrate(journal, target_version) -> Result<MigrationReceipt, JournalError>`
3. `transact(expected_generations, mutations) -> Result<CommitReceipt, JournalError>`
4. `rebuild_control_snapshot(journal) -> Result<ControlSnapshot, JournalError>`
5. `publish_snapshot_after_commit(snapshot) -> Result<SnapshotPublishReceipt, JournalError>`
6. `quarantine_on_identity_or_corruption_mismatch(reason) -> QuarantineReceipt`

## Required invariants

- only H5.1 bounded tables exist in baseline
- hot query admission performs zero redb writes
- snapshot publication follows committed transaction and never precedes it
- incarnation/route mismatch quarantines instead of attaching
- large point lists and bytes live in CAS manifests, not redb

## Typed failure surface

- `CONTROL_STORE_CORRUPT`
- `CONTROL_SCHEMA_MISMATCH`
- `CONTROL_IDENTITY_MISMATCH`
- `CONTROL_TRANSACTION_CONFLICT`
- `SNAPSHOT_PUBLICATION_FAILED`

## Exit tests / evidence

- `migration_golden_fixtures`
- `power_loss_reopen_matrix`
- `hot_query_does_not_mutate_redb_10000`
- `mismatched_route_quarantined`
- `forbidden_corpus_payload_guard`
- `snapshot_matches_committed_generation`

## Suggested internal modules

```text
search-control-redb/src/
  schema.rs
  migration.rs
  tables.rs
  transaction.rs
  snapshot.rs
  quarantine.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 7,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- If migration code or a table family grows independently, split internal modules first; a new crate requires an independent runtime/replacement boundary.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.
