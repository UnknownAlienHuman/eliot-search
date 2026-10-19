# search-control-redb

Small durable **current application state** in redb. Source bodies, prepared bodies, vectors, ranked
query windows and completed-operation history do not belong here.

Use the [architecture](../../docs/architecture/README.md) and
[implementation guide](../../docs/implementation/README.md). [FUNCTIONS.md](FUNCTIONS.md) records the
current implementation and the exact cutover.

## What the product needs

Typed tables for the state that must survive restart:

```text
applied settings
selected roots and current source heads
current preparation references
CurrentPublishedView
one unresolved PendingPublication
small owner/version metadata required to open the current root
```

Each ordinary update is one short redb write transaction. Filesystem and Qdrant effects happen outside
the writer transaction. A read path uses a redb read transaction or a narrow in-memory current pointer;
it does not rebuild another database-shaped snapshot after every mutation.

## Current source problem

`PersistentControlJournal` is a generic operation journal over `RECORDS` and `OPERATIONS`. Every accepted
mutation appends a receipt. The history is never pruned and eventually reaches fixed lifetime limits.

After a committed mutation, `publish_snapshot_checked` reads every record, copies all keys/values into a
new `ControlSnapshot`, validates it and swaps an in-memory pointer. Recovery verifies the complete current
record set and then scans the accumulated `OPERATIONS` table to find the receipt for the current
generation. Thus a small source-head or setting change is proportional to all control records, and restart
cost grows with completed history.

The generic pending mutation also acts as a whole-store availability fence. This is much wider than the
actual state that may be uncertain.

## Replacement

- Replace generic `ControlMutation`/receipt replay with typed table operations used by real callers.
- Load current state once at startup. Update only the affected cache/current pointer after commit, or read
  directly from an MVCC read transaction.
- A definitely aborted transaction returns its original failure. If commit acknowledgement is genuinely
  uncertain, reopen redb and inspect the affected typed key/current revision.
- Keep durable pending state only for a real external operation that can outlive the process, chiefly
  Qdrant publication. Its artifact/cursor belongs to the publication owner.
- Delete completed operation rows instead of compacting them into another permanent journal.
- Remove migration, DEK, provider-authority and security-restriction tables with their final product
  consumers when those superseded subsystems are removed.

Keep schema/version refusal, exact current-state validation, short atomic transitions and content-free
errors. Do not store source bytes or pretend redb can transact with Qdrant or the filesystem.
