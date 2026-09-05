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

The primary daemon has not yet migrated its existing file-based control state to this adapter.
Native file/root admission, migration, cancellation/deadline composition, capability-specific payload
codecs, pruning and full P02 qualification remain unfinished. The T09 touched-key increment changes no
public method signature, schema, digest, dependency or lockfile. Its 14 additional real-file regression
tests cover bounded lookup work, exact replay/recovery and corruption; they have not been compiled or
executed in the authoring environment. No T09 or product acceptance is claimed.

See [disk format, integration boundaries and verification](../../docs/runtime/CONTROL_REDB.md),
[function contract](FUNCTIONS.md) and [agent instructions](AGENTS.md).

```sh
cargo +1.98.0 test -p search-control-redb --lib --locked
cargo +1.98.0 check --workspace --all-targets --all-features --locked
```
