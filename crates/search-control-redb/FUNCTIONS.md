# search-control-redb — source and replacement map

The package currently mixes a useful redb current-state owner with several retired lifecycle platforms.
This document identifies what exists and what the connected application should keep.

## Current generic journal

| Source/API | Actual behavior |
|---|---|
| `PersistentControlJournal::open` | Opens/initializes the journal and may permit redb's native recovery. It is not a side-effect-free inspector. |
| `transact` / `recover` | Validate a caller mutation, write `RECORDS`, append one `OPERATIONS` receipt and update the header. A possible outcome occupies the package-wide pending slot. |
| `verify` | Reads and validates current records plus operation/header consistency. |
| `snapshot` / `publish_snapshot_checked` | Enumerate all `RECORDS`, copy them into a `BTreeMap`, rebuild `ControlSnapshot` and publish the new `Arc`. This runs after every committed mutation. |
| `recover_publication_checked` | Verifies current state, then iterates `OPERATIONS` to locate the receipt whose `after_generation` equals the current generation. |
| operation limits | Cap total operation count and receipt bytes over the lifetime of the journal; completed entries are not removed. |

Consequences:

```text
ordinary mutation cost  = O(all current control records)
recovery lookup cost     = O(all retained operation receipts)
maximum useful lifetime  = bounded by historical operations, not current state
one unknown mutation     = broad package availability fence
```

These costs are properties of Eliot's wrapper, not requirements imposed by redb.

## Other current modules

`access_policy`, `provider_authority`, `security_restriction`, `revision_dek`, `migration`, publication
phase codecs and quarantine helpers implement earlier programmes. Their typed codecs and fixtures do not
make them required owners for the local baseline.

Keep only modules with a real remaining current-state consumer. Do not preserve a package API solely so
old migration, grant, key-lifecycle or receipt tests continue compiling.

## Target typed operations

Implement narrow operations in the existing package or inline them in the control adapter:

```text
read/write applied settings
read/write root registration and current source head
read/write current preparation reference
read/write CurrentPublishedView
read/write/update one PendingPublication cursor
read small root/schema identity
```

Rules:

1. Open the relevant table(s) in one short transaction.
2. Check expected-current values when concurrent writers can conflict.
3. Commit, close the transaction, then update the narrow in-memory pointer/cache.
4. Never hold the writer across source-object or Qdrant I/O.
5. Never copy every table merely to publish one changed value.
6. Never retain a success receipt after no recovery or idempotency consumer remains.

### Failure semantics

- validation failure or explicit abort before commit: known failure;
- commit returned success: current typed state is authoritative;
- commit acknowledgement genuinely uncertain: reopen and inspect the affected key/revision;
- possible external Qdrant effect: keep publication's exact pending artifact and resolve external state;
- ordinary read/query cancellation: no durable control record.

A process-local current pointer may accelerate the published view or settings, but redb remains the durable
owner. The pointer is reconstructed from committed typed records at startup, not from a receipt scan.

## Removal boundary

After direct callers move, delete:

- generic `ControlJournal` mutation/receipt API and `OPERATIONS` table;
- full `ControlSnapshot` reconstruction after every write;
- lifetime receipt-count/receipt-byte ceilings;
- package-wide pending mutation for unrelated typed state;
- superseded migration, DEK, provider-grant and security-restriction persistence;
- source-shape/receipt fixtures whose only purpose is to preserve those APIs.

Retain focused tests for atomic current-state updates, expected-current conflict, restart readback,
unsupported schema and the real pending publication transition.
