# search-control-redb — source and replacement map

The package currently mixes useful redb transactions/point reads with a generic journal, snapshot
publisher and several retired lifecycle programmes. Keep the primitives; remove the operating system.

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

## Useful primitive already implemented

[`persistent/snapshot/lookup.rs`](src/persistent/snapshot/lookup.rs) contains
`read_current_records_checked`. It already does the small operation the replacement needs:

1. validate a finite distinct key set;
2. open one redb read transaction;
3. verify the current header;
4. point-read only those keys from `RECORDS`;
5. return the common generation and bounded values;
6. perform no table scan, write, operation lookup or snapshot swap.

Use this shape for typed settings/root/source/view reads. It does not need `ControlSnapshotPublisher` or a
copy of every control record. Where direct typed tables replace the generic `RECORDS` encoding, preserve
the same one-transaction point-read behavior.

## Registry adapter on the current path

The normal daemon does not use a small current-state adapter. `direct_registry_command` and
`native_source_command` call `bind_original_v4`, then:

```text
restore full SourceRegistry from registry operation history
→ load/replay the selected operation
→ retain/reconstitute original policy/admission input layouts
→ prepare/commit one generic registry batch
→ read every changed target back
→ drop the port
→ publish a rebuilt full ControlSnapshot
```

`RegistryJournal::bind` without original-v4 handling was found mainly in its tests. Supporting non-test
files under `bins/eliot-searchd/src/registry_control/` already exceed about 63 KiB before the large main
adapter; they exist to retain originals, restore v4 batches and reproduce source IDs. The command still
ends with no retained source revision.

This is pre-release replay/migration machinery in the normal command path. Do not optimize it. Replace it
with typed current-state rows and remove the wrapper.

## Other current modules

`access_policy`, `provider_authority`, `security_restriction`, `revision_dek`, `migration`, publication
phase codecs and quarantine helpers implement earlier programmes. Their codecs and fixtures do not make
them required owners for the local baseline.

Keep only modules with a real remaining current-state consumer. Do not preserve a package API solely so
old migration, grant, key-lifecycle or receipt tests continue compiling.

## Target typed operations

Implement narrow operations in this package or its direct daemon adapter:

```text
read/write applied settings
read/write RootRecord
read/write SourceRecord + current RevisionRef
read/write current PreparedArtifactRef
read/write CurrentPublishedView
read/write/update one PendingPublication cursor
read small root/schema identity
list a bounded page of current roots/sources/references
```

Rules:

1. Open the relevant table(s) in one short transaction.
2. Check expected-current values only where concurrent writers can conflict.
3. Commit, close the transaction, then update the narrow in-memory current pointer/cache.
4. Never hold the writer across source-object or Qdrant I/O.
5. Never copy every table merely to publish one changed value.
6. Never retain a success receipt after no recovery/idempotency consumer remains.
7. Resolve source registration/update by the actual current root/source/head rows, not a historical
   operation envelope or restored original admission transcript.
8. Use one consistent read transaction for publication/inventory snapshots; do not reconstruct a
   second mutable in-memory database from all previous operations.

### Failure semantics

- validation failure or explicit abort before commit: known failure;
- commit returned success: current typed state is authoritative;
- commit acknowledgement genuinely uncertain: reopen and inspect the affected typed key/revision;
- possible external Qdrant effect: keep publication's exact pending artifact and resolve external state;
- ordinary read/query cancellation: no durable control record.

A process-local current pointer may accelerate the published view or settings, but redb remains the durable
owner. The pointer is reconstructed from committed typed records at startup, not from a receipt scan.

## Removal boundary

After direct callers move, delete:

- generic `ControlJournal` / `ControlJournalPort` mutation-and-receipt API and `OPERATIONS` table;
- `ControlSnapshotPublisher`, full `ControlSnapshot` reconstruction and generation receipt lookup;
- `RegistryJournal`, v4 batch restore, original-input retention/reconstitution and source-ID replay;
- lifetime receipt-count/receipt-byte ceilings;
- package-wide pending mutation for unrelated typed state;
- superseded migration, DEK, provider-grant and security-restriction persistence;
- source-shape/receipt fixtures whose only purpose is to preserve those APIs.

Retain focused tests for atomic current-state updates, point reads from one generation, expected-current
conflict, restart readback, unsupported schema and the real pending publication transition.
