# search-control-redb

Bounded technical control state, never a searchable corpus.

`PersistentControlJournal` is the concrete disk-backed adapter using pinned redb 2.6.3 without Python
features. It implements atomic record/receipt transactions, exact-input replay protection, unknown
commit recovery, read-only snapshots, current-receipt publication and explicit owner-epoch handoff.
Vendor types remain private. The previous `ControlJournal` reference model and its public API are
preserved in `src/reference.rs`; it is not a fallback for an unavailable disk backend.

Normal mutations plan final record/byte counts from only the distinct touched keys, then verify the
exact new header, receipt, written classes/bytes and deleted-key absence. They no longer copy or scan
all unrelated records twice per write. The database remains privately owned by this journal; no second
mutable catalog is introduced. Full structural verification still runs on create/open, explicit `verify`
and recovery; an explicit snapshot request still reads the whole bounded snapshot.

Replay validates the receipt's operation, command, generation and complete changed-key binding. When
that receipt names the current generation, replay/recovery also verifies the actual values and absence
of deleted keys. A historical receipt does not reapply or compare obsolete values against newer writes.
Detected persistent record/schema/binding corruption quarantines mutation use. An uncertain commit
keeps the exact-operation pending fence until recovery. These checks do not make the redb file an
authenticated ciphertext format or allow other processes to modify it outside the owning adapter.

The public `ControlSnapshotPublisher` fences generation, owner epoch and immutable journal identity
before updating the existing snapshot pointer. Equal-generation content or operation conflicts are
rejected. Disk publication and disk recovery use this same public boundary. See
[readback and publication fences](../../docs/runtime/CONTROL_READBACK_FENCES.md).

## Context-controlled calls

`read_snapshot_with_context`, `verify_with_context`, `transact_with_context` and
`recover_transaction_with_context` consume the existing `search_ports::OperationContext`.
One monotonic relative deadline starts at call entry and spans all internal phases; cancellation
is checked before dispatch, between bounded records and before returning a complete result.
No partial snapshot is returned. Existing byte/item ceilings remain enforced; the opaque
`budget_ref` is not decoded or treated as authority.

The function contract's dispatch boundary is preserved: interruption after a write transaction
begins is `CONTROL_COMMIT_OUTCOME_UNKNOWN`, even if an explicit staged abort succeeds.
The exact request stays pending until a fresh recovery call verifies its committed receipt or
absence. After possible commit, interruption likewise cannot release the fence or imply rollback.
Interrupted or transiently unavailable recovery never clears the fence and never becomes a false
corruption verdict. Actual record/schema corruption still quarantines. Failed recovery permits
readback only, not a blind mutation retry. Historical replay does not restore obsolete values.

`ControlCallError` carries the closed journal reason, shared failure/retry classifications,
interruption cause and exact requested `MutationId` (redacted in Debug). No operation identity is
fabricated or converted to the shared port's distinct opaque ID. The old methods remain compatible
unscoped low-level entrypoints and use the same algorithms, not a second implementation.

Checks are **cooperative**, not a hard wall-clock I/O guarantee: redb 2.6.3 and synchronous OS calls
cannot be preempted by these probes. Expiration is detected when they return. No detached worker or
unbounded queue is introduced to disguise that limit. Daemon hard-timeout composition remains open.

The primary daemon has not yet migrated its file-based control state to this adapter. Full
`ControlJournalPort` binding, context-controlled create/open/owner handoff and snapshot publication,
native file/root admission, migration, capability-specific payload codecs, pruning and P02
qualification remain unfinished. No shared trait, disk schema/codec, digest, dependency, lockfile,
workflow or qualification gate changes here.

Sixteen new tests use real journal files with deterministic cancellation and fake-clock checkpoints:
pre-dispatch refusal, staged abort plus absence recovery, post-commit interruption, interrupted and
transient recovery, actual corruption, empty/partial scans, historical replay and accumulated deadlines.
They are application-boundary tests, not physical-power-loss tests. New and existing Rust suites have
**not been compiled or run** in the authoring environment, where cargo/rustc and working download DNS
were unavailable. No T09 completion, green build or product acceptance is claimed.

See [disk format, integration boundaries and verification](../../docs/runtime/CONTROL_REDB.md),
[function contract](FUNCTIONS.md) and [agent instructions](AGENTS.md).

```sh
cargo +1.98.0 test -p search-control-redb --lib --locked
cargo +1.98.0 check --workspace --all-targets --all-features --locked
```
