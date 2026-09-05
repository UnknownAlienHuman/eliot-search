# Disk-backed technical control journal

`search_control_redb::PersistentControlJournal` performs real redb file I/O. The unchanged in-memory
`ControlJournal` is retained in `src/reference.rs`; it is not a fallback for the disk adapter. Neither
implementation is a search index. The primary daemon still uses its existing file-based control state;
this increment does not migrate or switch that state.

## Dependency and boundary decision

The adapter pins `redb = 2.6.3` with default features disabled and `sha2 = 0.10.9`, already present in the
workspace lockfile. No `python`, `pyo3`, logging, networking or alternate-index feature is enabled.
redb owns transactions/pages; SHA-256 binds actual canonical requests. The existing declared BLAKE3
command digest remains a separate field and is never populated with SHA-256 output.

The pinned redb registry checksum is
`8eca1e9d98d5a7e9002d0013e18d5a9b000aee942eb134883a82f06ebffb6c01`.
Sources: upstream `cberner/redb` tag `v2.6.3` (`Cargo.toml`, `src/db.rs`, `src/transactions.rs`) and the
`rust-lang/crates.io-index` entry `re/db/redb`. This is an implementation pin, not a latest-version claim
or Windows qualification receipt. All other locked package versions are unchanged.

The disk adapter exposes existing package-owned records and standard `File`, never redb types. The
caller must supply a verified final regular-file handle, retain the external root-owner guard for the
journal's entire lifetime and validate capability-specific payload schemas. An opaque `ControlValue`
class tag does not by itself prove that arbitrary bytes are content-free. No source bodies, extracted
text, postings, vectors, credentials or query history belong in these records.

## Implemented operations

`create(file, identity, limits)` initializes only an explicitly newly created empty file. `open` requires
an existing non-empty database and checks the exact installation incarnation, data-root ID, owner epoch,
path digest and schema family/version. The two entrypoints do not silently replace each other. Empty,
missing-table, extra-table, incompatible-schema or inconsistent state is refused, not repaired into an
empty installation.

`transact` commits technical records, monotone data generation, bounded counters and one operation
receipt together with `Durability::Immediate`. Cache allocation is fixed at 8 MiB. Application storage
uses exactly three private tables: `eliot.control.meta.v1`, `eliot.control.records.v1` and
`eliot.control.operations.v1`. This is the adapter's bounded v1 schema, not acceptance of the full
architecture H5 table inventory.

Replay checks the actual canonical request, including operation ID, immutable journal identity,
expected data generation, declared digest, sorted writes/deletes, record classes and exact values.
Reordering distinct writes is equivalent. Changing a value, class or generation while retaining the
same declared digest is an operation conflict. Duplicate touched keys are rejected.

After commit, exact header, receipt and resulting record readback precede success. A commit or readback
failure leaves the handle blocked as `CommitOutcomeUnknown`; `recover_transaction` resolves the same
complete request from disk without executing it again. A different request cannot clear the block.
Recovering an old receipt does not restore old values over a newer committed transaction.

`read_snapshot` is read-only. `control_snapshot` reconstructs the shared immutable snapshot.
`publish_committed_snapshot` verifies a real current disk receipt before using the existing publisher.
`recover_snapshot_publication` republishes current state without replaying its write. An initialized
empty journal has no mutation receipt and returns `None`, not fabricated acceptance evidence.

`advance_owner` is an explicit consuming handoff to the next epoch under the caller's newly verified
owner guard. It cannot change incarnation, root, path or schema. Data generations and old operation
receipts remain intact. Request fingerprints exclude only the changing live owner epoch; the header
fences that epoch. Failed possible writes consume the handle and require exact reopening. `open` itself
never guesses a successor or changes ownership.

## Limits and incomplete integration

`JournalLimits` bounds records, keys, values, mutation items and receipt count. Each encoded receipt must
also fit `max_value_bytes`; total receipt bytes have their own ceiling equal to
`max_total_value_bytes`, separate from live record bytes. A full ledger refuses new writes; this version
does not prune replay protection. Per-handle commit counters are diagnostics, not disk-I/O measurements.

No automatic schema migration, application integrity repair, receipt pruning or runtime capability
acceptance is added. redb's own unclean-transaction recovery is not a proof of Windows power-loss safety.
The adapter uses synchronous local I/O; the full deadline/cancellation port wrapper remains unfinished.

Before switching the primary daemon, composition must bind real persistent installation/root identities,
verify final file/root ownership on the target platform, use the explicit owner handoff and migrate the
existing control records with exact source/target readback. A fresh empty redb database must never be
advertised as the migrated state of an existing data root. Native race, crash/power-loss, migration,
retention and full P02 acceptance remain outstanding. The DIRECT preparation and Qdrant data-plane gaps
are not closed by this backend.

## Verification

```sh
cargo +1.98.0 check --workspace --all-targets --all-features --locked
cargo +1.98.0 test -p search-control-redb --lib --locked
```

The new suite contains 21 regression cases plus one ignored subprocess helper. It covers real file
reopen/replay, exact-input conflict, rollback, lost acknowledgement, historical recovery, atomic
record changes, 10,000 read-only snapshots with byte-for-byte database comparison, identity mismatches,
second-handle rejection, corrupt/unknown schema, receipt capacity, snapshot publication and owner
handoff. The subprocess case exits without destructors immediately before and after commit, then
reopens and compares the committed state. Its ignored child is invoked by the parent test automatically.

These Rust checks were not executed in the authoring environment: no Rust toolchain or working outbound
DNS was available. TOML parsing, the exact lockfile dependency delta and file/blob identity were checked;
those checks are not compilation or test execution. No gate, artifact qualification or passing test
receipt is issued by this change. GitHub workflows remain manual-only.
