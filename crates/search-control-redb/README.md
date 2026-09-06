# search-control-redb

Bounded technical control state, never a searchable corpus.

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
commit/publication with request admission and consult the owning publisher for each admission. An
already returned Arc or a cloned publisher is a historical view, not a live subscription, grant or
revocation mechanism. Independent journal writes do not notify arbitrary cached snapshots.

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
