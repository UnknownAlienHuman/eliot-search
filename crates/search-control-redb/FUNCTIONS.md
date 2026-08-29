# Function contract — `search-control-redb`

**Status:** W1/P02 logical contract; no redb dependency, schema migration or fault evidence is accepted.

This package owns the bounded durable control journal and immutable control-snapshot reconstruction. It
is never a searchable corpus and stores no source bodies, extracted text, postings, vectors or ordinary
query history.

## Global rules

- one journal identity binds installation incarnation, data-root owner epoch, schema family/version and
  exact local path identity;
- every mutation uses stable operation/idempotency identity and expected generation guards;
- reads are side-effect free; hot request admission creates zero redb writes;
- large exact point/object sets remain immutable CAS manifests referenced by digest;
- committed durable state precedes immutable in-memory snapshot publication;
- corruption, identity mismatch or unverified migration quarantines rather than guesses/repairs in place;
- vendor redb types never cross the public API.

## Configuration operations

### `section_descriptor() -> ConfigSectionDescriptor`

### `compiled_defaults() -> ConfigSectionInput`

### `validate_section(input, platform, accepted_capabilities) -> Result<ValidatedControlConfig, JournalError>`

Implements `config/sections/control.md`. Durability and verified-or-quarantine migration policy are fixed
floors. Paths must stay under the current owned data root.

### `section_digest(validated) -> Blake3Digest32`

### `plan_section_change(old, new) -> Result<SectionReloadDecision, JournalError>`

Opening/path/schema/durability changes require dependency restart or rejection. Bounded idempotency TTL/
capacity and snapshot timing may apply live only without weakening correctness.

## Journal identity and open

### `derive_journal_identity(root_owner, path, schema) -> Result<JournalIdentity, JournalError>`

Purely binds owner/data-root/incarnation/path/schema identities. A path string alone is insufficient.

### `inspect_journal(path, platform_port, deadline, cancel) -> Result<JournalObservation, JournalError>`

Observes existence, file identity, header/schema/integrity and lock state without migration or repair.
Partial/cancelled inspection cannot authorize open.

### `open_or_create(config, owner_guard, expected_identity, operation, deadline, cancel) -> Result<JournalGuard, JournalError>`

**Preconditions**

- exact live owner guard matches root/incarnation;
- config/schema descriptors are validated;
- no conflicting journal guard/operation exists;
- migration plan is either unnecessary or separately accepted.

**Postconditions**

- created/opened file identity and header match expected identity;
- one process-local guard owns the vendor handle;
- required tables/indexes and schema version are verified;
- returned guard exposes vendor-neutral read/mutation ports only.

Timeout after creation/open metadata mutation is unknown until exact inspect/reopen. Identity mismatch or
corruption returns quarantine, never attach.

## Migration

### `plan_migration(observed, target, registry) -> Result<MigrationPlan, JournalError>`

Produces an explicit ordered plan with source/target schema digests, preconditions, bounded steps,
rollback/backup policy and golden fixture identity. Unknown versions have no implicit migration.

### `execute_migration(guard, plan, operation, deadline, cancel) -> Result<MigrationReceipt, JournalError>`

Runs each verified step transactionally where supported and records step/plan identities. Cancellation
occurs only at declared safe points. Unknown outcome is resolved by schema/table/data invariant readback.
A partially unprovable migration quarantines.

### `verify_migration(guard, plan, fixtures) -> Result<MigrationVerificationReceipt, JournalError>`

Reopens/reads the migrated journal and proves schema/table/record invariants plus migration digest.
Success is required before normal snapshot publication.

## Read and mutation transactions

### `read_snapshot(guard, request, deadline, cancel) -> Result<JournalReadSnapshot, JournalError>`

Returns a consistent vendor-neutral read view with journal generation/identity. It performs no durable
write, idempotency row, lease or query-history mutation.

### `transact(guard, command, expected_generations, operation, deadline, cancel) -> Result<ControlCommitReceipt, JournalError>`

Validates a bounded closed command set, operation identity, expected table/entity generations and
immutable manifest refs before entering one atomic transaction.

Success returns exact journal/transaction generation, changed entity refs and content-minimized receipt.
Same operation plus same canonical command returns the prior receipt; same operation plus different input
is `CONTROL_OPERATION_CONFLICT`.

Timeout/cancellation after transaction dispatch is `CONTROL_COMMIT_OUTCOME_UNKNOWN`. Recovery performs
idempotency/record/generation readback; it never blindly repeats under a new identity.

### `recover_transaction(guard, operation, expected_command_digest) -> Result<CommitRecoveryDecision, JournalError>`

Returns exactly:

```text
COMMITTED(receipt)
NOT_COMMITTED_RETRY_SAME_OPERATION
CONFLICTING_INPUT
PARTIAL_OR_CORRUPT_QUARANTINE
```

## Control snapshot

### `rebuild_control_snapshot(read_snapshot, schema) -> Result<ControlSnapshot, JournalError>`

Purely reconstructs the immutable bounded control snapshot from committed technical records. It rejects
missing/duplicate/incoherent generations, unresolved publication intent and oversized/unbounded state.
It never scans Qdrant to infer control truth.

### `publish_snapshot_after_commit(commit, snapshot, publisher, deadline) -> Result<SnapshotPublishReceipt, JournalError>`

Requires snapshot journal generation/identity equal the committed state and atomically publishes the
immutable in-memory snapshot. Publication cannot precede commit.

If publication fails after commit, durable state remains authoritative and request admission stays
fail-closed/degraded until `recover_snapshot_publication` succeeds.

### `recover_snapshot_publication(guard, publisher) -> Result<SnapshotPublishReceipt, JournalError>`

Reads current committed state, rebuilds the exact snapshot and republishes it. It never replays the
original mutation solely to obtain an in-memory receipt.

## Idempotency maintenance

### `prune_idempotency(guard, policy, protected_operations, deadline, cancel) -> Result<PruneReceipt, JournalError>`

Deletes only expired bounded operation rows not required by unresolved/recoverable commands. Pruning is
background technical maintenance and cannot make an unknown external mutation unrecoverable.

## Quarantine and diagnostics

### `quarantine(reason, observation, owner_guard) -> QuarantineReceipt`

Produces content-minimized reason/identity/digest metadata and blocks normal mutation/open. It does not
delete or auto-repair the journal.

### `journal_health(guard_or_observation) -> ControlStoreHealth`

Reports identity/schema/generation/migration/snapshot state, bounded counts and reason codes. It excludes
source/query content and unrestricted paths.

## Cancellation, deadline and crash semantics

Open, migration, transaction, pruning and snapshot publication have finite deadlines and cooperative
cancellation at safe points. Any possible durable mutation is unknown until exact journal readback.
Power loss/reopen must produce committed prior state, committed new state or explicit corruption/
quarantine—never fabricated success.

## Typed failures

- `CONTROL_STORE_UNAVAILABLE`
- `CONTROL_STORE_CORRUPT`
- `CONTROL_STORE_QUARANTINED`
- `CONTROL_IDENTITY_MISMATCH`
- `CONTROL_SCHEMA_UNSUPPORTED`
- `CONTROL_SCHEMA_MISMATCH`
- `CONTROL_MIGRATION_UNVERIFIED`
- `CONTROL_TRANSACTION_CONFLICT`
- `CONTROL_GENERATION_MISMATCH`
- `CONTROL_OPERATION_CONFLICT`
- `CONTROL_COMMIT_OUTCOME_UNKNOWN`
- `CONTROL_READ_CANCELLED`
- `CONTROL_BUDGET_EXCEEDED`
- `SNAPSHOT_REBUILD_FAILED`
- `SNAPSHOT_PUBLICATION_FAILED`
- `FORBIDDEN_CONTROL_PAYLOAD`

## Required tests / qualification evidence

- create/open/reopen exact identity and second-handle/process denial;
- table/schema descriptors contain only bounded technical control state;
- forbidden source/text/vector/ranked-query payload compile/runtime guard;
- migration golden fixtures for every supported source version;
- crash/power-loss before/after each migration/transaction/snapshot boundary;
- same-operation replay and conflicting command rejection;
- 10,000 hot query/read admissions produce zero redb writes/idempotency rows;
- large exact sets stored as immutable manifest refs, not redb arrays;
- Qdrant state cannot reconstruct or override control truth;
- committed-but-unpublished snapshot recovers without replaying mutation;
- idempotency pruning preserves unresolved operation recovery;
- corruption/identity mismatch/migration ambiguity quarantines;
- `control` config fixed durability/migration floors and change classification;
- public API/vendor-type and debug/content-disclosure guards;
- fake storage/publisher/clock ports prove orchestration independence.
