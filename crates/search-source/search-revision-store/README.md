# search-revision-store

One current-format immutable object reader/writer for retained source and prepared artifacts.
Source/head semantics stay in source/control owners; preparation schema stays in materializer/unitizer.

Follow the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and [source map](FUNCTIONS.md).
Integration waits for #598's accepted public handoff and reuses its useful retained-source result rather
than creating a competing writer.

## Current problem

The package contains a real immutable-file kernel, an in-memory revision lifecycle state machine and
legacy inventory/migration code. The adjacent materializer contains another nearly identical artifact
publication kernel.

The contextual source-object kernel is about 40 KiB before tests; materializer's contextual artifact
kernel is also about 40 KiB. Both model detailed work phases, paired context errors and receipts around
the same write/sync/publish/readback operation.

The contextual publication path intentionally performs no stage unlink on success, error or `Drop`.
A successful object can therefore leave its staging name behind. Materializer repeats the same behavior.
Staging disposition is not durable source truth and should not become a permanent recovery subsystem.

## One object algorithm

```text
verified owned bytes
→ unique create-new stage in the final directory
→ bounded write + file sync
→ no-clobber publication to the content-addressed final name
→ exact existing/final verification
→ platform-qualified namespace durability
→ remove stage
→ ObjectRef
```

If another writer wins the same final name, verify exact kind/profile/length/content and remove this
writer's stage. Different bytes at one address are an immutable conflict. If a crash leaves a stage or an
unreferenced final object, ordinary bounded GC may remove it after current, pending and pinned references
are known. Unknown cleanup leaks bytes safely; it does not block serving a verified final object or require
a lifetime receipt journal.

The repository already contains a useful internal donor in
`search-control-redb/src/migration/cutover_artifact.rs`: create-new temporary write, file sync, no-clobber
hard link, temporary removal, directory sync and final readback. Extract that small mechanism; do not keep
the migration platform that currently owns it.

## Windows durability is an explicit profile decision

The existing daemon adapter's `sync_directory_result` is a real directory `sync_all` on Unix but a no-op
that returns success on non-Unix. Exact post-publication readback establishes the current namespace state;
it does **not** by itself prove that the name survives a sudden power loss.

The Windows implementation must therefore select and qualify a supported same-volume no-clobber/
write-through finalization primitive, or explicitly report a weaker crash-durability profile. A candidate
is native move/rename with write-through and no replace, but its exact NTFS/ReFS semantics, antivirus/file-
sharing behavior and crash result must be exercised on supported Windows. Never represent the current
no-op directory sync as durable success and never add a receipt platform to conceal the missing primitive.

## Reads

Reopen by `ObjectRef`, not by the live source path. Verify the current object format and actual integrity.
Support full and bounded range reads under one deadline/cancellation context and count all bytes scanned.
When integrity requires a complete streamed pass, share that verification across selected ranges from the
same object instead of rereading the object for every excerpt.

Storage protection is a selected concrete profile. It does not require universal cross-user key
migration, key history, retention leases or secure-erasure proofs.

## Removal

After callers move, delete old DIRECT/materializer stores, duplicate contextual kernels, in-memory
operation/tombstone histories and legacy inventories that have no current-format reader. Unsupported old
roots fail before writes and remain intact.
