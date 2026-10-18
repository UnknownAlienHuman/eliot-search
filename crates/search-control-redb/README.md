# search-control-redb

Local control persistence over redb. This package stores current source/head metadata, settings,
publication state and references to immutable artifacts. Source bodies, prepared text, vectors and
query history belong elsewhere.

Follow the [implementation guide](../../docs/implementation/README.md#3-keep-control-state-small-and-publication-real).
The changes below are the implementation target; this documentation does not claim that the Rust
backend has already been simplified.

## Existing code

| Owner | What currently exists |
|---|---|
| [PersistentControlJournal](src/persistent.rs) | A real disk backend with create/open, transactions, read snapshots and recovery. |
| [Transaction engine](src/persistent/transaction.rs) | Conditional record updates, request fingerprints, permanent operation receipts and post-commit readback. |
| [Registry adapter](src/persistent/registry.rs) | Atomic batches and targeted state/operation reads over source-owner metadata. It does not acquire or retain source bytes. |
| [Publication records](src/persistent/publication.rs) and [visibility](src/persistent/publication/visibility.rs) | Persistent intent, visibility and recovery operations. Actual Qdrant effects remain daemon/bridge work. |
| [Snapshot publication](src/persistent/snapshot.rs) | An immutable in-memory view reconstructed from committed database state. |
| [ControlJournal](src/reference.rs) | An in-memory reference implementation; it is not a persistent backend or an outage fallback. |

Schema versions 1–5, Direct/Indexed/MigrationInert profiles, reserved envelope codecs and staged import
APIs describe the existing implementation. They are not frozen requirements for the new format.
The full staged migration platform and [file-journal publication model](src/publication_codec.rs)
are retirement candidates with their consumers, not mandatory next work.

## Replace the transaction platform with direct control operations

Keep this existing storage owner and implement typed operations for the records the application uses.
Within one short redb write transaction, read the required conditions, update the affected records and
commit. Use read transactions for consistent snapshots. Keep vendor handles private; do not add another
database, generic command/receipt framework or persistence wrapper for each table.

A retained object must exist before its revision/source-head reference commits. The source owner
supplies identity and revision semantics. The storage owner enforces the atomic update; it does not
re-evaluate source admission or hash source bodies.

Remove the permanent archive of completed local operations. The current
`transaction::plan_commit` equates operation count with generation; baseline limits are 65,536
operations and 64 MiB of operation bytes. The disk adapter does not prune this history. A small live
catalog can therefore stop accepting new updates because its completed history filled the ledger.
Use a bounded retry window only where a real client retries mutations. Retain actual unfinished work,
not every past receipt.

## Distinguish an aborted transaction from an unknown external effect

- A successful abort before commit leaves no committed application update. Return the underlying
  cancellation, limit or transaction failure and release that operation's local pending state.
- A possible commit with an unresolved result needs inspection of actual committed state.
- A possible Qdrant mutation needs the separate pending publication record and real indexed-state
  readback. A redb abort cannot undo an already dispatched Qdrant write.
- Ordinary reads and pure transformations create neither operation records nor durable receipts.

The current `execute_scoped` returns `CommitOutcomeUnknown` for cancellation or budget exhaustion
even after `write.abort()` succeeds. Remove that false uncertainty while preserving recovery for
an actual unresolved commit or external write.

The shared publication coordinator remains the sole publication transition owner. Daemon composition
persists its pending intent here, performs Qdrant writes/readback outside the redb writer, then commits
the visible route/epoch and publishes the in-memory view. Remove the duplicate daemon
`Publisher`/`FileJournal` path; a supplied observation is not a database commit.

## Formats and verification

The new backend may reject an old root before writes and leave it intact. New roots and generations
rebuild from available sources. No old-schema support, adoption chain, export implementation or
migration issue must precede the new application. Original files and sole retained copies are user
data and are not deleted by this replacement.

Verify actual atomic changes, a known abort, an unresolved commit, restart, pending external
publication and updates after the retry window fills. Read/query admission must not write control
history. Keep useful bounds and corruption errors; remove tests that only preserve obsolete schema
bytes, receipt shapes or migration stages.

The pinned [redb 2.6.3](https://github.com/cberner/redb/blob/v2.6.3/README.md) already supplies
`begin_write`, typed tables, `commit` and `begin_read`. It does not make files and Qdrant part of one
transaction. See [FUNCTIONS.md](FUNCTIONS.md) for concrete entrypoints to change.
