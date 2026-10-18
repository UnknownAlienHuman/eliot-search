# search-revision-store

Retain exact source bytes and reopen the revision that supplied a result.

Read the [implementation guide](../../../docs/implementation/README.md) for the connected source,
control and publication path. [FUNCTIONS.md](FUNCTIONS.md) maps the existing code;
[AGENTS.md](AGENTS.md) describes changes in this owner.

## Current code

| Area | What exists |
|---|---|
| [Immutable object I/O](src/immutable_object.rs) | Real bounded filesystem reads, staging, no-clobber publication and final readback. The daemon supplies platform observations. |
| [Revision kernel](src/kernel/store.rs) | An in-memory state machine for append, recovery and deletion, with operation/deletion histories and tombstones. This is not a persistent source catalogue. |
| [Backend contract](src/kernel/backend.rs) | `RevisionObjectBackend` describes encrypted writes/readback. There is no production implementation connected to the new ingestion path. |
| [Legacy inventory](src/legacy_inventory.rs) | DIRECT filename classification, inventory, checkpoint/page and report code. These are old-format consumers, not requirements for new storage. |

The daemon's [storage adapter](../../../bins/eliot-searchd/src/secure_direct_store_storage_io.rs)
already calls the immutable-file APIs. Its existence does not connect native acquired bodies to a
durable new revision/source-head record.

## Implementation target

Use one existing immutable-byte writer/reader for retained objects and prepared artifacts. Source IDs,
revision references and source-head changes belong to the source/control path; materialization formats
belong to preparation. Different payload meanings do not justify two filesystem engines.

The current [revision I/O kernel](src/immutable_object/context.rs) and materializer's
[artifact I/O kernel](../../search-prep/search-materializer/src/legacy_artifact/context.rs) duplicate
staging, reads, linking and durability. Consolidate their useful mechanics without introducing a
universal storage abstraction.

Write and verify the object before the short source-head transaction. Reopen actual stored bytes with
their expected length/digest when serving evidence. Transfer the owned buffer to its consumer.

Do not connect the old in-memory operation archive, mandatory encrypted payloads or residency domains
merely to satisfy their constructors. The new format may reject an old root before writing and leave it
intact. Original files and sole retained copies remain user data.
