# Function contract — `search-control-redb`

## Selected-reference finite use (#598)

The additive `source_retention` profile reserves `secret/revision-use/v2/`.
Generic writers cannot write or delete it. Explicit NoMaterial genesis requires
proven absence of original DEK rows; an exact Core Intent is then bound before
credential effects. Missing usage state never proves a fresh key. One complete
residency claim and cumulative attempts/GHASH/plaintext charges remain permanent.
`RevisionUseCommand` prepares only from scoped readback with explicit missing
keys. The original-context port uses the existing conditional transaction engine;
only a fresh acknowledged reservation yields a non-Clone
`CommittedRevisionSealUse`. Replay and recovery return no use permit. Dropped,
cancelled and uncertain uses remain charged. Complete original canonical AAD and
occurrence metadata stay in the operation-addressed reservation, never plaintext.
Windows must reconstruct and compare the entire AAD before consuming the permit.
Native freshness/access, exact key selection, crypto validation and immutable
object publication remain with their actual owners. This source seam alone does
not qualify the complete consumer or paired restore continuity.

The revision-DEK candidate adapter reserves `secret/revision-dek/v1/{intent,candidate,current}`.
Ordinary generic writers cannot touch that namespace. The closed original-context
route permits one canonical State Intent when all three are absent, then one
Candidate against the exact Intent while Candidate/Current remain absent. It
never writes Current or deletes keys. Core owns full record semantics. Targeted
operation-existence reads validate the existing immutable operation metadata and
perform no writes; a positive result fences entropy, not serving authority.

Normal DIRECT/registry adapters may borrow the original absolute deadline and
work callback through the checked owner facades. Every admitted read/copy/hash/
write site retains cumulative accounting and first refusal; unknown commit
suspends publication until exact operation/state recovery. These facades share
the existing engines and codecs, not a second journal or state machine.
The Direct initialization operation binds complete actual JournalIdentity and
the original request through the shared canonical owner. No Native/root/access
authority is created by a storage profile or restored metadata.

### C07 derived-artifact v1

The additive `derived_artifact` profile uses the same selected-v2 usage row, selected Current record,
residency claim, attempt/block/plaintext caps, checked GCM charge, and original-context transaction
engine. It does not append `SourceHistory` or mutate any raw selected-v2 key/value bytes. Its explicit
`secret/revision-use/v2/derived-artifact/v1/` records are versioned rebuild state.

The Root caller supplies `DerivedArtifactScopeGuardKeysV1` in Root-owner, source-revision,
namespace-owner, residency-policy, purge-fence order. It reads the returned
`derived_artifact_reservation_keys(...)` through `read_control_keys_with_original_context` so usage,
material intent, selected Current, three derived-record absences and all five exact guard values come
from one coherent disk read. `RevisionUseCommand::next_counter` remains the only counter reader. After
the parent constructs the C07 descriptor and final AAD for that counter, it passes
`DerivedArtifactUseMetadataV1` to `DerivedArtifactUseCommand::reserve`. The reservation conditionally
writes the shared usage counters, a versioned subject row, a versioned exact Current/AAD/charge row, and
the five captured guard values in one existing-journal transaction.

`BoundControlJournal::transact_derived_artifact_use_with_original_context` returns a
`CommittedDerivedArtifactSealUse` only after a fresh acknowledged commit. The permit is non-Clone and
exposes `metadata()`, `selected_current_canonical()`, `counter()`, `aad_canonical()`,
`plaintext_length()`, and `authenticated_blocks()`. Its checked charge is exactly
`ceil(P/16) + ceil(AAD/16) + 1`. Replay returns no permit; recovery through
`recover_derived_artifact_use_with_original_context` returns only the existing recovery decision.
Unknown/refused use stays charged and never authorizes blind re-encryption.

After AES and C07 reference/address derivation, the caller builds
`DerivedArtifactObjectPublicationV1` from the exact canonical object-reference bytes, address bytes,
actual stored-object digest and stored-object length. `DerivedArtifactUseCommand::bind_object_publication`
creates a separate immutable late-intent command conditioned on the exact subject/reservation rows,
selected Current and the same five Root/source/namespace/policy/purge values. The caller commits it via
`transact_derived_artifact_object_intent_with_original_context` before calling C07 filesystem publish.
Its recovery method returns no permit. `DerivedArtifactReservationReadback::read` and
`DerivedArtifactObjectIntentReadback::read` expose equality metadata; `binds` joins reservation
coordinates, while the C07/Root verifier must compare the opaque descriptor/reference/AAD and validate
actual source/root/policy state. C02 does not parse the C07 codec or claim a stored-byte digest proves
owner authenticity.

## Prepared artifact-set reference v1 (#260 C02 link)

`PreparedArtifactSetRecordV1::prepare_with_original_context` encodes bounded
`PreparedArtifactSetMetadataV1`: original Root operation, full `SourceRevisionRef`,
Root-issued materialization/representation IDs as data, complete opaque six-domain
canonical bytes, complete opaque C07 set-object reference bytes, and actual stored
set-object digest/length. C07 reference bytes are capped at 20 KiB, residency at
8 KiB and the full C02 record at 48 KiB, within unchanged journal limits.
Generic canonical framing is checked; no C07 codec, body or dependency is added.

The reference is `c02:prepared-artifact-set:v1:<full-lowercase-digest>`, computed
through C00 over the actual full canonical C02 record under the fixed domain
`eliot/raw/c02-prepared-artifact-set-record/v1`. It names the immutable State row
`publication_intents/prepared-artifact-set/v1/<digest>` in the existing records
table. Preparing this candidate proves no persistence or C07 I/O. It is never a
synthetic publication/write/read receipt, semantic digest or profile identity.

The caller explicitly sets `PublicationIntent.prepared_manifest_ref` to that
record's reference, then calls `with_prepared_artifact_set_v1(record)` on the
existing `PublicationIntentUpdate` or `PublicationSuccessor`. First begin and
successor insert the record with absence conditions in the same transaction as
`INTENT_DURABLE`, before the caller may mutate Qdrant. Advance compares the exact
record bytes. A present row is not overwritten by a different operation; exact
original-command replay compares the complete existing request fingerprint.
No reference is silently rewritten or interpreted as this profile by shape alone.

`VisibleEpochCommit::with_prepared_artifact_set_v1(record)` requires every new
membership manifest pointer in this publication to name the attached record.
Retirement-only changes retain their original references. Full record bytes,
existing intent, visibility, guards, previous pointers and matching shadows are
atomic conditions of the existing VisibleEpoch transaction. One set per intent
is supported; aggregation and membership/partition qualification remain Root work.
The existing intent, visibility, receipt, current-manifest and retired-reference
encodings are unchanged. The new record/API is explicitly v1 and opt-in on
visibility schemas 4/5; no migration or second journal/map/catalogue is created.

The original-context bound port exposes publication transact/recovery, visible
commit/recovery, intent-head read, snapshot publication and
`read_prepared_artifact_set_with_original_context`. All borrow the original
absolute deadline and cumulative callback. The typed read returns actual journal
identity/generation and exact record metadata from one point read. Record codec,
hash and copy work are conservatively prepaid (16 times the fixed record ceiling
for encoding, and 16 times stored length before decoding); these are logical
admission bounds, not CPU/RSS measurements. Generic writers cannot touch the new
record namespace. Disk verification/restart/snapshot reconstruction check full
record hashes and reject dangling v1 intent, receipt, current or retired refs.

The prior individual-object reservation and late object intent are not reused as
this observation: they precede C07 filesystem publication and do not retain an
actual verified set-object observation. Root must supply and compare the actual
C07 write/read receipt, source/MatRep provenance, full residency and current
owner/policy/purge state. This C02 record is technical evidence only; no query
writes, retention-root qualification, Native/Qdrant/product proof or #260 closure
is claimed. The local C07 `DERIVED_ARTIFACT_V1.md` and maintainer request define
this slice; the remote issue body was not fetched under the no-remote boundary.

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
`prepare_indexed_profile_adoption` captures the actual immutable DIRECT profile and
current control generation. `adopt_indexed_profile` conditionally writes one immutable
original-input adoption record plus the supplied epoch-zero visibility in the SAME
journal. It never rewrites genesis or source metadata. `recover_indexed_profile_adoption`
resolves only the original command; replay/conflict/unknown retain existing semantics.
Disk/snapshot verification joins both immutable records to their actual transactions.
Root must verify the real qualified route/schema and current owner/source/access
guards before preparation; these technical APIs do not establish Qdrant readiness.
MigrationInert adoption still refuses. Native v2 creation and source admission remain
Root responsibilities; this package mints none of those authorities.

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

`LegacyInventoryEncoder::item_encoded_len(family, status, kind, counts)` sizes
one item without allocation through the actual encoding formatter; digest fields
have fixed widths. `preflight_remaining(pending_records, pending_encoded_bytes)`
returns `LegacyInventoryCapacity` after the live emitted prefix, pending items,
and conservative remaining terminal/footer bounds. Include all pending fanout,
synthetic Gap24/348 and other future items; do not add terminals/footer twice.
The query is allocation-free and non-mutating, and does not reserve capacity or
issue authority. Unknown items need valid upper bounds. Tail overestimation can
refuse an exact-fit stream; the actual encoder retains its unchanged hard limits.

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

`SourceImportRecordArtifactPublication::begin_borrowed_rows_with_context` is an
opt-in sequential cursor over the retained original file. Its exclusive mutable
publication borrow prevents another owner cursor or publication from sharing the
offset; each pass seeks to zero. It creates no OS handle and charges no cursor
`Open`. The default `begin_rows_with_context` still clones/charges its handle.
Both use the same bounded row/chain/count/byte/EOF, metadata, native identity and
fresh staging/final locator guards with the original deadline/work callback.
Rows precede final proof: a short/dropped/refused cursor grants no completed pass.
Drop only releases the borrow/reader; it never writes or removes retained evidence.

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

## Coherent current projection references

`BoundControlJournal::read_publication_projection_snapshot_with_original_context`
returns a privately constructed `PublicationProjectionSnapshot` from one verified
redb read transaction: journal identity/generation, committed visibility, current
projection-membership manifest references and source-specific shadows. It reuses
the existing record codecs and receipt-ledger verification, rejects unresolved
publication work and mismatched owner guards, and borrows the original deadline
and work callback. It creates no records, durable query job or second store.

The returned references are immutable technical observations. Root must reopen
and verify the exact C07 manifests and join actual C03 membership and C18 live
authorization before compiling retrieval/IDF/count populations. The snapshot
itself does not authorize access or prove Qdrant/source availability. Existing
recovery checkpoints still retain unresolved work for recovery; their semantics
and all persisted profiles remain unchanged.
