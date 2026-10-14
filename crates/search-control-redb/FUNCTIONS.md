# Function contract — `search-control-redb`

## Frozen schema-5 operating profile

- `JournalOperatingProfile`: closed Direct / Indexed / MigrationInert observation.
- `JournalProfileInitialization`: Direct, Indexed with actual supplied visibility,
  or MigrationInert; no default, readiness boolean or caller-created verified state.
- `initialize_operating_profile`: exact generation-zero conditional commit of the
  immutable reserved profile and, only for Indexed, existing validated visibility.
- `recover_operating_profile_initialization`: inspect the exact original command;
  preserve historical replay and unknown/conflict classifications without rewriting.
- `operating_profile`: bounded current technical observation, never serving authority.

Direct registry/control writes need no Qdrant route. Indexed visibility/intent paths
retain their semantic validation. MigrationInert cannot bind normal registry or
publish/admit a current snapshot. Generic mutation cannot write/delete the profile.
The original profile codec/operation join is checked on disk verification and snapshot
publication. Schemas 1/2/3/4 and v1 bootstrap semantics remain unchanged.
Later Direct-to-Indexed and inert adoption explicitly refuse until a separately
accepted exact owner/source/access-counter transition contract exists. Root owns
native v2 creation and source admission; this package mints none of those authorities.

## Concrete registry reservation (#269)

- `RegistryMutationRequest::new`: finite owner-supplied metadata and exact before
  values; keeps original journal generation and separate registry revision.
- `ControlPortCommand::Registry`: typed route through the existing borrowed
  journal/publisher port, with snapshot suspension after dispatch.
- `PersistentControlJournal::commit_registry_batch`: one conditional atomic
  transaction, immutable full-ID operation and exact target readback.
- `recover_registry_batch`: original-command recovery; no blind re-execution.
- `lookup_registry_operation`: targeted immutable lookup/proven absence with
  actual identity, journal generation and registry revision coordinates.
- `read_registry_operation_targets`: exact envelope and all requested targets
  from one coherent read, with no catalog/history scan or semantic callback.
- `limits`: actual executing bounds copied from the journal, also on its bound port.
- `read_registry_snapshot`: complete coherent technical restart observations;
  source-owner semantic restore and admission decoding remain Root's responsibility.

Schema 5 is VERSIONED_REBUILD. Reserved codecs also run during open/recovery and
snapshot verification; old-schema collisions refuse. Existing seven classes,
three physical tables and active limits remain unchanged. Wrapped state and
operation values must fit the 64 KiB baseline; no silent splitting or limit rise.
The unshipped schema-5 family map includes Root tag 4 with Identity class. Root
key/value bytes remain source-owner validated opaque metadata; tags 1/2/3 and
the original-operation codec/join remain unchanged.

**Status:** concrete disk persistence and schema-5 registry reservation are implemented.
Complete #268 candidate adoption, #269 serving cutover and native/fault/installed
qualification remain separate, incomplete work.

Inventory codecs select one explicit LegacyInventoryProfile. Existing constructors
and descriptor bytes retain V1 basename semantics; LocationV2 selects distinct
header/footer names, descriptor and artifact suffix. The original publication
attempt stores that selected descriptor, and its opaque completed proof selects
both same-database adoption and independent readback decoders. Header/profile
mismatches refuse rather than reinterpret retained V1 references.

V2 physical subjects are correlation digests over bounded root-relative normal
native components with explicit platform, count and length framing. Kind 16
binds failed directory observations, kind 36 successful ones, and kind 37 pairs
a preparation-reference locator with its semantic key and exact evidence digest.
These records grant no source, cleanup or cutover authority.

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

## Versioned legacy-inventory stream

The original inactive inventory receipt may continue into an explicit content
stage over the same retained database. Only an original-bound, once-published
source-content v1 artifact admits bounded row/index adoption. Both canonical wire
rows and their technical identity index require independent readback, and every
canonical occurrence is joined against exact legacy source/revision, SHA and length
fields. Header/footer bytes remain owned by the frozen encoder and strict decoder.
The extended content table profile is unavailable to old or normal journal opens.
The final original-context checkpoint must succeed before a content owner becomes
writable. A terminal refusal retains the target/publication evidence and blocks
later publication, row adoption and readback even if the same callback later permits work.
The receipt remains partial: semantic candidate, serving, restart, residency and
cleanup authority are not issued; #268/#269/#347/#348/#215 remain separate owners.

`LegacyInventoryEncoder::{header,item,source_event,end_family,finish}` provides
one bounded metadata encoder for `eliot.legacy-inventory.v1`. Family order and
per-status counters belong to this owner. Complete accounting requires all
12 terminals and finite record/byte limits; gaps, unreadable and unsupported
observations cannot prove a complete denominator. Original native custody,
physical enumeration, revision acquisition and protection remain composition
observations, never codec-issued authority. Existing `SourceImportRecordArtifact`
owns staging, complete second-stream comparison and once-only publication.
No footer authorizes #268-C acceptance, #269 cutover, #348 restart reconciliation
or #347 cleanup. Native/fault/installed qualification remains #215.

## Contextual immutable record lifecycle

`SourceImportRecordArtifact::{create,push,freeze}_with_context` and contextual
readback, comparison, preparation and publication methods borrow the existing
`MigrationBootstrapWork` callback together with the original `Instant`. The
deadline-only APIs use the same kernel with an inert callback. No callback,
context or renewed allowance is retained in an artifact.

`Open`, `Read`, `Write`, `Allocate`, `Process` and `Checkpoint` work is
admitted before the corresponding inner operation. Reader processing is charged
per actual refill, row copying and hashing separately; interrupted I/O attempts
require fresh admission. The private writer flushes explicitly and never on Drop.
Pre-effect refusal prevents link dispatch; post-effect refusal is unknown and
preserves original identity, locators and the actual reused outcome. Native
composition retains the first original refusal cause. Exact inspections and
same-process-only resolution are unchanged; this adds no cleanup or restart
authority.

The record platform's contextual directory/locator/identity/durability methods
must expose nested native work. Simple observer ports have compatibility defaults;
qualified native ports override them. The daemon admits each actual locator or
directory open, metadata observation and Windows identity dispatch, with original
checkpoints between them. Legacy native methods use the same engine with an inert
hook; no second observer, deadline or allowance is created.

## Original migration bootstrap

`MigrationBootstrap::create` initializes the caller-created single future inactive
target and writes the bounded `eliot.migration-bootstrap.v1` original operation
through the existing immediate conditional transaction/readback engine. It retains
the actual RELEASED source read-custody observation, all admitted limits, original
registration/source snapshot and explicit native output identity profile. No normal
journal handle or candidate escapes. Native BLAKE3 and a borrowed checkpoint/work
callback remain composition supplied; the callback cannot renew the deadline.

`publish_inventory` takes the retained original publication object, writes its
exact kind/profile/stage/final/native identity/chain/rows/bytes and dispatch phase
before at most one link, then records the owning same-process durable outcome.
Any uncertain journal phase forbids dispatch; any post-effect refusal stays unknown
and retains evidence. Reuse/restart/candidate/cleanup authority is unavailable.
The package-owned receipt explicitly reports `complete_candidate_not_performed`.
The legacy import table/META profiles remain unchanged. A separately reviewed
versioned migration must continue complete mapping in this same inactive target.

## Live owned source stage

`MigrationBootstrap::into_source_stage` requires the retained successful inventory
outcome and exact original target/source snapshot plus the accepted mapping profile.
`OwnerBoundSourceStage::{publish_source_map,initialize_mapping,push,begin_readback}`
records the original immutable map attempt before once-only publication and creates
mapping tables in the existing database. Its combined seven-table inactive profile
requires the exact finite `owner_source_stage.v1` marker on writes and readback.
Legacy control/import profiles and normal serving admission remain unchanged.

`OwnerSourceMappingReadback::{compare,finish}` verifies the complete row denominator,
all indices and five accounting dimensions under the borrowed original context.
Any comparison error latches; retry or finish cannot produce a receipt. The receipt
retains the original database owner and reports
`complete_inventory_candidate_not_performed`. It grants no full inventory adoption,
cutover, serving, restart reconciliation or cleanup authority.

## Original all-row inventory stage

`OwnerSourceMappedReceipt::into_inventory_stage` consumes the original owner;
`OwnerInventoryStage::{push,begin_readback}` decodes and batches every canonical
inventory row in that same database. It seals only the exact original published
chain/rows/bytes, with encoder-derived complete family accounting. No caller
summary, current path or digest can construct this state.

`OwnerInventoryReadback::{compare,finish}` compares a complete regenerated stream,
the persisted full denominator and every source-event index binding. Errors latch
and cannot produce `OwnerInventoryMappedReceipt`. The receipt exposes all five
source counts and all twelve family status arrays; it grants no semantic candidate,
normal journal, serving, restart or cleanup authority. Old four/seven-table profiles
remain separately admitted. Original work/deadline/cancellation applies to every
buffer/decoder/hash/read/write phase. Native execution remains #215.
