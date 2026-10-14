# search-control-redb

Bounded technical control state, never a searchable corpus.

## Registry reservation and atomic storage (#269)

Disposition: **VERSIONED_REBUILD**, journal schema **5**. The three physical tables
and seven record-class tags are unchanged. Schema 5 adds the exact reserved
namespace `eliot.control.registry.v1/`; old schemas refuse colliding records on
open/readback and cannot execute registry commands. There is no in-place adoption
or automatic migration. The schema descriptor binds the prefix and wrapper codecs.
Schema 5 has a mandatory, package-owned operating-profile record. Schemas 1/2/3/4
and the legacy v1 migration bootstrap retain their existing semantics.

### Frozen operating-profile contract

This slice starts from c5556f3 and changes only this package. The unshipped schema-5
descriptor versions the reserved operating-profile key and codec before integration.
`JournalOperatingProfile` is the closed Direct / Indexed / MigrationInert enum.
`JournalProfileInitialization` selects Direct, Indexed with an explicitly supplied
`PublicationVisibilityState`, or MigrationInert. `initialize_operating_profile`
atomically installs the immutable profile at generation zero; Indexed installs its
validated initial visibility in the same transaction. Exact initialization recovery
uses the original command and operation identity and never repeats a write.

Direct permits bounded registry/control metadata and published technical snapshots
without any indexed route. It rejects indexed intent/visibility/manifest/receipt
keys. Indexed requires the complete existing visibility validator. MigrationInert
permits only inert Migration-class control writes and refuses normal registry ports,
provider admission and current serving snapshots. Its source/inventory/content table
capabilities remain private to the original live migration owner.

Generic writes/deletes cannot replace the operating-profile record. Open, explicit
verification, recovery and snapshot publication validate its codec and original
generation-zero operation receipt. A nonempty schema-5 journal without the record
refuses; no automatic adoption or default mode exists. Binding/dispatch retains the
same publisher suspension and exact disk-generation checks.

Direct-to-Indexed and MigrationInert adoption remain explicit refusals in this slice.
Control has no accepted authoritative source/access-counter binding with which to
validate a later transition. Existing visibility initialization cannot seed a route
after Direct writes or invent collection IDs, counters or policy authority. Native
v2 creation and actual source-owner admission inputs remain Root's integration work.

Root constructs `RegistryStateKey` from a closed family and the source owner's
complete canonical key bytes, and `RegistryStateChange` from exact before/after
metadata bytes. Source, membership and generation use Identity, State and Revision
respectively. Root metadata uses Identity and storage family tag 4; existing
Source/Membership/Generation tags 1/2/3 stay unchanged. Root key/value bytes remain
opaque here and require source-owner/daemon semantic validation. This extends the
unshipped schema-5 descriptor, without creating schema 6 or adopting unknown tags.
`RegistryMutationRequest::new` retains the complete original opaque
operation ID, original journal generation, independent registry revision,
owner-computed command digest and canonical operation/delta envelope.

`PersistentControlJournal::commit_registry_batch` uses the existing conditional
kernel to compare exact classes/bytes or absence and atomically write every state,
the immutable operation envelope and registry revision. Generic mutations cannot
write/delete reserved keys. `ControlPortCommand::Registry` exposes the same path
through `BoundControlJournal`, suspending snapshot admission until verified disk
publication. The raw ID remains in the envelope; SHA-256 is only its technical
backend index and is never relabelled BLAKE3 or used as source/admission authority.

Normal commit readback reads the exact operation, revision and requested targets
from one transaction, without rebuilding the full catalog/history. The separate
`read_registry_snapshot` is a complete bounded startup/recovery read. It joins
operation envelopes to the actual backend receipt ledger and exposes original
owner bytes for Root's semantic decoders. `recover_registry_batch` requires the
original command/generation; historical replay acknowledges the old receipt while
returning current targets, without writing obsolete values back. Late readback
failure retains the original pending fence and cannot unblock another mutation.

All wrapped values, including the complete immutable operation envelope, must fit
the active journal value limit (baseline 64 KiB). Canonical keys plus storage
framing must fit the key limit. For D targets, writes plus conditions consume
`2*D+4` items. Refusal occurs before dispatch; no limit inflation, truncation,
splitting an atomic batch or additional database is provided. A source-owner
256 KiB envelope ceiling does not imply it fits this storage profile.

This package validates technical namespace/class/framing/bounds and atomicity.
Root must validate actual PreparedBatch/key/state/admission semantics, provide
original owner-issued admission inputs, restore source-owned state, and perform
the verified #268/#269 native cutover. No source-registry dependency, admission
evaluator, source-byte acquisition, serving fallback or qualification is added.
Disk reopen/lost-acknowledgement component fixtures do not qualify native custody,
power loss, external publication outcome or installed serving.

`PersistentControlJournal` uses pinned redb 2.6.3 without Python features. It provides atomic
record/header/receipt transactions, exact-input replay protection, unknown-commit recovery,
read-only snapshots and explicit owner-epoch handoff. Vendor types stay private. The in-memory
`ControlJournal` in `src/reference.rs` remains a test/reference implementation, never a fallback
for an unavailable disk backend.

Normal mutation planning and readback inspect distinct touched keys rather than copying the entire
record set. They verify final counters, the exact receipt, written classes/bytes and deleted-key
absence. Full structural verification runs on create/open, explicit `verify` and recovery. Snapshot
requests read the complete bounded live inventory.

Replay binds the actual canonical request, not just a caller-supplied digest. Current-generation
replay/recovery verifies the actual values and deletions; historical replay neither compares old
values against new ones nor restores them. Uncertain mutations remain pending until exact recovery.
The disk file is not an authenticated ciphertext format; this adapter assumes exclusive ownership.
Capability owners must validate payload semantics: a technical class tag alone does not prove that
arbitrary bytes are content-free.

### Targeted registry reads

The normal registry adapter uses `lookup_registry_operation(id, context)`, then
decodes immutable owner bytes outside the database and calls
`read_registry_operation_targets(envelope, keys, context)`. Both methods also
exist on `BoundControlJournal`. `RegistryOperationLookup::Absent` carries actual
identity/journal-generation/registry-revision coordinates; `Committed` contains
`RegistryOperationObservation` with those current coordinates and the unchanged
original `envelope()`. The second call rechecks that exact envelope and all keys
in one read transaction. Missing targets, changed envelopes, duplicates, active
bounds and pending/unknown state refuse without partial output or false absence.
Neither normal lookup scans the full catalog/history. Missing revision metadata
requires one bounded empty-prefix observation; operation absence checks the real
backend receipt as well. `limits()` on journal/bound port returns the executing
profile for adapter preflight. The complete startup snapshot API remains separate.

## Record-artifact retention (#345)


The original staging `File` is retained from creation through freeze, exact replay,
publication, synchronization and readback. Record-artifact Drop never removes a
locator; successful publication reports `DeferredTo347` cleanup. The package has
no qualified original-object unlink operation. Even pre-publication errors retain
staging instead of attempting the old check-then-path-unlink cleanup.

`SourceImportVerifiedRecordArtifact::prepare_publication` creates a private-field
`SourceImportRecordArtifactPublication` containing the original handle/native
identity, original local staging/final names and exact chain/row/byte counts.
`publish_once` permits one no-clobber link dispatch. Before dispatch the disposition
becomes `RetainedUnknown`; every uncertain link, directory sync, readback, identity
or late-deadline result preserves it. `resolve_same_process` never links: it requires
that live original object, performs fallible synchronization and post-sync exact
readback, and can finish once with `VerifiedCleanupDeferred`. Equal-byte objects
with a different original identity cannot complete the operation.

The existing consuming `publish` adapter uses this same one-shot implementation.
Its daemon consumers remain `control_migration_plan.rs` and
`control_migration_content.rs`; their existing unknown reasons and output frames
are unchanged. Dropping the adapter on an unknown result loses its in-memory proof
but retains filesystem evidence. A known conflicting final observed after dispatch
also remains unknown; it is never overwritten or deleted.

`classify_source_import_record_artifact` reports only current evidence: Missing,
StageExact, FinalExact, BothExactSameObject, BothExactDistinctObjects, Conflict,
Replaced or Unavailable. Its result always reports `Unresolved` and has no retry,
success or cleanup API. `classify_current` additionally checks the live operation's
original identity but still grants no authority. Losing the original operation,
including process restart, cannot remint it from current names, bytes or identity.
Durable request/root-owner/publication-phase linkage remains #348 under #268/#269.
No proof sidecar, identity serializer or migration schema is added.

The record-specific platform seam requires a fallible directory flush. The daemon
uses a read/write Windows directory handle with `FILE_FLAG_BACKUP_SEMANTICS` and
`sync_all`, or the existing Unix directory sync. The bounded current-token Windows
probe of that call succeeded; this is not power-loss or installed qualification.
Focused owner fixtures use injected identities and sync failures; losing their
operation models proof loss within one process, not an executed process restart.
Native replacement/deletion/power-loss qualification remains #347/#215. Record
rows, chains, domains, final names, source-map/content receipts and inactive redb
bytes remain unchanged.

## Original-context record I/O (#497)

The record lifecycle's `*_with_context` methods borrow the caller's original
`MigrationBootstrapWork` callback and deadline for creation, buffering, exact
readback and publication. Inventory and owned source-stage composition use these
methods through their existing original allowance. Legacy deadline-only methods
delegate to the same implementation with an inert callback.

The kernel admits each actual open, read refill, write attempt, allocation and
processing pass before work, with checkpoints around native observations, sync
and link effects. Interrupted reads and writes retry only after fresh admission.
Short rows share one admitted reader refill. The explicit writer buffer performs
no I/O on Drop, so refusal cannot trigger a later implicit flush.

The native record platform overrides the simple-port contextual adapters. Its
locator verification admits the actual file open separately from directory/file
metadata and opened-handle observations. Directory synchronization admits its
directory open and flush separately. The existing Windows identity reader polls
and admits each of its three native dispatches; after-dispatch polls do not charge
another observation. Inert callers reuse those same physical implementations.

Publication becomes retained unknown before its last pre-link admission. A late
refusal retains both locators and the actual new/reused link outcome for the
existing same-process resolution. It cannot authorize a second dispatch or
cleanup. Complete identity, chain, row, EOF and durability checks remain in place;
all full inspections are still required and charged. Focused component fixtures
exercise local file I/O with injected identities; native cancellation, fault and
installed qualification remain #215.

## Complete inventory metadata adoption (#509)

The original source-mapped receipt can consume itself into `OwnerInventoryStage`.
It adds two bounded metadata tables to the same inactive database, under the
explicit nine-table profile. Normal control and old import opens reject this
profile. The strict inventory decoder reuses the existing H/I/E/T/F encoder;
every row, all twelve terminals and the footer must re-encode exactly.

Adoption uses 256-row batches and binds the full stream chain, row count and byte
count to the original published inventory. Readback compares every independently
recompiled row and verifies each source event through the existing source-map
indices and all five accounting dimensions. Refusal latches. A duplicate terminal
after all families refuses rather than indexing past the family array.

The opaque receipt reports all family counts and denominator outcomes, plus the
remaining `complete_semantic_candidate_not_performed` gap. Metadata references do
not substitute for semantic root/publication/security adoption, a verified cutover
candidate, restart linkage or cleanup. Native/installed qualification remains #215.

## Atomic record preconditions

`ConditionalControlMutation` combines an existing `ControlMutation` with bounded
`ControlRecordCondition::exact(key, value)` or `::absent(key)` assertions. The public
`transact_conditionally` and `recover_conditional_transaction` entrypoints require an
`OperationContext` and use the same transaction/recovery engines as ordinary mutations.

Conditions compare exact classes and bytes, or actual absence, first in the coherent planning read
and again inside the write transaction before any record changes. A false precondition returns
`CONTROL_GENERATION_MISMATCH` without a commit or receipt. Global expected generation is still
mandatory; an A/B/A transition cannot pass an older command merely because values became equal again.
Condition-only keys are not listed as changed entities. A command with no writes/deletes is refused.

The full condition set is bound to the actual request fingerprint using the domain
`eliot-search/control-conditional-request/sha256/v1`. It includes the existing request SHA-256,
condition count and sorted length-delimited keys, absence/exact tags, classes and expected values.
No BLAKE3 field is populated with this SHA-256. Reordering distinct conditions is equivalent; adding,
removing or changing any condition conflicts with a saved operation even when its declared digest is
unchanged. Duplicate condition keys are refused; a condition may concern a key the mutation changes.
Empty conditions preserve existing request fingerprints exactly. Old binaries can read unchanged
journal framing, but cannot replay a nonempty conditional command through their unconditional API.

Replay is not a second execution of the old preconditions. At the current receipt generation, changed
keys must match the mutation's post-state and untouched condition keys must still match their asserted
state. Contradiction is corruption, not a new failed CAS. Historical replay does not test obsolete
conditions against newer values. Recovery after abort, lost acknowledgement, reopen or owner handoff
requires the exact original condition set; stripping it cannot clear pending state.

Writes + deletes + conditions share `max_mutation_items`. Expected values have the existing individual
value ceiling and an aggregate ceiling of `max_total_value_bytes`. Validation, hashing, comparisons
and recovery share one cooperative deadline. Normal work touches only changed/condition keys, not all
unrelated records. The existing file/table/receipt layout and unconditional reference model are unchanged.

This primitive supplies atomic technical comparisons, not the full `VisibleEpoch` protocol. It neither
adds the H5 table inventory nor decides semantic owner/source/membership/access guard completeness.
Those codecs and the actual live authorization barrier must be supplied before `ControlJournalPort`
can be accepted. No new journal owner, mutable catalog or daemon cutover is introduced.

Twenty-four conditional regression tests cover exact/absence/class guards, all-or-nothing batches,
ABA, ordering, changed/stripped conditions, bounds, write-transaction revalidation, interruption,
reopen/handoff, current versus historical recovery, corruption and touched-key work. A known-answer
fingerprint was independently calculated; the Rust tests themselves remain **NOT_RUN** without Cargo.

## Operation contexts

The existing `OperationContext` controls `read_snapshot_with_context`, `verify_with_context`,
`transact_with_context`, `recover_transaction_with_context`, `create_with_context`,
`open_with_context` and `advance_owner_with_context`. One monotonic relative budget spans each call;
checks do not reset between stages. The opaque `budget_ref` is not decoded or treated as authority.
Old low-level entrypoints use the same algorithms with an unscoped compatibility policy.

Interruption after write-transaction dispatch is `CONTROL_COMMIT_OUTCOME_UNKNOWN`, even after an
explicit successful staged abort. Only fresh exact recovery can establish commit or absence.
Interrupted/transient recovery neither clears the pending fence nor invents corruption. Actual
record/schema corruption still quarantines. Typed failures preserve exact mutation identity with
redacted Debug output; no shared opaque ID or digest conversion is fabricated.

Lifecycle calls return no usable guard after incomplete initialization, inspection or handoff.
The caller supplies a verified regular-file handle and retains the external root-owner guard.
`create` accepts an explicitly new empty file; `open` refuses empty existing files and never creates
missing application tables. Native open may recover redb metadata, so interruption/transient failure
after dispatch requires exact inspect/reopen. This is not side-effect-free `inspect_journal`.

Owner handoff consumes the old journal, allows only the next epoch or verified no-op, and preserves
data generations and prior receipts. Lifecycle `MutationId` is error correlation only, not a new
idempotency-ledger row or ownership grant. Reopening is resolved by the exact prior/intended header
identity; a matching header cannot grant external root ownership.

## Snapshot publication and admission

`control_snapshot_with_context`, `publish_committed_snapshot_with_context` and
`recover_snapshot_publication_with_context` complete the context-controlled snapshot surface. Their
old entrypoints delegate to the same checked paths. Receipt, records and recovery ledger are read
through one coherent redb read transaction. Reconstruction uses the existing pure validator.

A matching disk publication/recovery attempt suspends `ControlSnapshotPublisher` admission before
inspection, including a pre-cancelled call. On failure, `current()` returns `None` and
`requires_recovery()` is true. The sole previous Arc remains private for monotone identity, generation,
content and operation comparisons; hiding the admission view does not erase those fences. Foreign
journal identities or older owners are rejected before changing the correct publisher's state.

The fence retains the highest generation actually observed in a validated disk header, not an
untrusted receipt's claimed generation. Recovery cannot reset it with an older/empty journal. Once a
publisher is disk-bound, its caller-supplied snapshot and reference-model recovery entrypoints refuse
updates, even while otherwise ready. Only verified disk publication can reopen admission.

A pending mutation must be recovered first. Snapshot recovery cannot clear its operation fence, and
transaction recovery alone does not publish a snapshot. Snapshot publication/recovery dispatches no
durable write and never repeats a commit. An interrupted publication does not invent a new uncertain
commit; the error preserves the interruption and requires fresh readback. Generation zero returns
`None` rather than a fabricated mutation receipt and cannot replace a later published generation.

One cooperative deadline covers receipt readback, record/ledger scans, reconstruction and validation.
The last cancellation checkpoint is immediately before the pointer swap, after all fallible scans
and content comparisons. Success linearizes at that swap; cancellation arriving later cannot undo it
or turn it into an unknown storage effect. All required validation precedes reopening admission.

This is a process-local publication fence, not automatic daemon integration. The caller must serialize
commit/publication with request admission and consult the owning publisher for each admission.
`ControlSnapshotPublisher` is deliberately non-Clone. Previously a clone copied `disk.suspended`
and the admission pointer; suspending the original did not suspend that independently mutable copy.
It could continue returning `Some(snapshot)` without the required fresh recovery read. Calling the
copy a historical view did not remove its publication/admission methods.

Clone the immutable `Arc<ControlSnapshot>` returned by `current()`, not its owner. Moving the owner
preserves its pending fence; reconstructing admission requires the existing verified journal path.
Already returned Arcs remain historical data, not live subscriptions, grants or revocation mechanisms.
No new shared publisher state, mutex, storage format or implicit notification mechanism is added.
Independent journal writes still do not notify arbitrary cached snapshots. This change is not daemon
integration and does not establish cross-process owner exclusion.

Compatibility: removing the public Clone implementation intentionally rejects callers that duplicate
the mutable publisher. Existing publication methods and immutable snapshot types are unchanged.
A compile-fail doctest protects the non-Clone boundary; a real-redb regression fixture covers move,
suspension, immutable Arc cloning and verified recovery. Both new tests are NOT_RUN without Cargo.
The doctest requires `cargo +1.98.0 test --locked -p search-control-redb --doc`; `--lib` alone does not run it.

## Verification and remaining work

Checks are cooperative, not a hard wall-clock guarantee: synchronous redb/OS calls cannot be preempted.
No detached worker, extra journal, queue, schema/codec/hash change or dependency was added for contexts.
Native root/file admission, side-effect-free inspection, full `ControlJournalPort` binding,
capability-specific codecs, migration, safe idempotency maintenance and P02 qualification remain open.
The primary daemon has not yet migrated its file-based catalog to this adapter (T10/T11).

Seventeen snapshot regression tests were added: phase cancellation/deadlines, blocked admission,
no model bypass, forged/foreign receipts, empty and old-generation recovery, same-generation conflicts,
corruption/transient reads, pending mutation separation, owner/restart recovery, final-checkpoint races
and 10,000 read-only admissions. Existing transaction, context, lifecycle and snapshot-fence tests are
retained. These are application-boundary fixtures, not machine power-loss qualification.

Rust compilation and tests for this change are **NOT_RUN**: the local Cargo command was unavailable
(exit 127); archive acquisition failed. Source/blob checks are not execution or independent acceptance.
No T09 completion, gate advancement, Windows qualification or product-readiness claim is issued.

```sh
cargo +1.98.0 test -p search-control-redb --lib --locked
cargo +1.98.0 check --workspace --all-targets --all-features --locked
```

See [function contract](FUNCTIONS.md), [agent instructions](AGENTS.md),
[disk format](../../docs/runtime/CONTROL_REDB.md) and
[earlier readback fences](../../docs/runtime/CONTROL_READBACK_FENCES.md). The snapshot admission
semantics above supersede the earlier description that every rejected disk publication leaves the
prior snapshot available to new requests. The prior immutable value itself is still preserved.

## Streamed legacy inventory

`LegacyInventoryEncoder` owns the explicit `eliot.legacy-inventory.v1` metadata
profile. A header binds the original retained installation/root/operation,
legacy source snapshot, registration digest and finite limits. Entries are
streamed under twelve ordered families, seven closed statuses, twelve required
terminals and one accounting footer. Limits are 32 directory names per page,
8 KiB per row, two million records and 512 MiB encoded output. Exhaustion refuses
publication rather than truncating the denominator.

The native migration consumer retains one original request/deadline/cancellation
and selected existing protector. Directory handles stay pinned across every page;
reads use no-follow opens, exact same-handle observations and single-link objects.
The source/root/registration snapshot is revalidated, a complete second stream is
compared, and publication uses the existing once-only record-artifact lifecycle.
Unknown publication retains its evidence and remains unresolved.

The stream contains references, identities and counts. It contains no source
bodies, plaintext/key material or absolute local paths. Readable source events,
heads, retained revisions, directory generations and preparation bindings use
their existing owning codecs. Unmapped/unsupported objects and unavailable joins
remain explicit. Directory/profile gaps are never complete negative evidence.
The footer grants no verified-candidate, cutover, restart or cleanup authority;
#268-C/#348, #269, #347 and #215 remain separate product obligations. Existing
source-map/content/staged-plan bytes and receipts remain unchanged.

The separate `MigrationBootstrap` owner records the actual original operation and
inventory publication attempt in the single future inactive target, using the
existing three-table journal and immediate conditional readback. Its borrowed
native callback reserves finite logical work under the original absolute deadline.
The result is first-phase metadata, not a verified candidate. A failed/unknown
phase cannot dispatch; a possible publication followed by refusal retains evidence
and cannot retry. Old source-map/content/import/plan bytes remain unchanged.

`MigrationBootstrap::into_source_stage` consumes the completed live inventory
owner. The new inactive `eliot.owner-source-stage.v1` profile adds four mapping
tables to that same database, retaining the three original control tables and an
exact owner marker. Generic control and legacy import readers reject this profile.
The shared mapping engine imports bounded batches and compares every source row,
its indices and all five counts before returning a partial source-mapping receipt.
A first readback mismatch is permanent for that handle. No second database, normal
journal handle, serving snapshot, complete candidate or restart capability escapes.
