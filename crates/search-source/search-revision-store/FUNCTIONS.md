# Code map — search-revision-store

The [implementation guide](../../../docs/implementation/README.md) defines the target. This is an
inventory of actual functions, not a requirement to preserve the old encrypted/lifecycle model.

## Existing functions

| Location / symbols | Current behavior |
|---|---|
| [immutable_object.rs](src/immutable_object.rs): `read_legacy_revision_object`, `publish_legacy_revision_object` | Read bounded bytes and publish immutable files through supplied platform observations. |
| [immutable_object/context.rs](src/immutable_object/context.rs): `read_legacy_revision_object_with_context`, `publish_legacy_revision_object_with_context` | The same file mechanics with caller context, work counters and attempted-effect reporting. Contextual publication currently leaves its stage behind. |
| [LegacyRevisionObjectRead::into_bytes](src/immutable_object.rs) | Transfers the owned read buffer; it does not copy the whole object. |
| [kernel/address.rs](src/kernel/address.rs): `derive_object_address` | Derives the old residency-qualified address. |
| [kernel/store/append.rs](src/kernel/store/append.rs): `RevisionStore::prepare_append`, `confirm_append` | Mutates in-memory Pending/Active state and retained operation receipts using supplied intent/readback. Does not write a file or commit redb. |
| [kernel/store/recovery.rs](src/kernel/store/recovery.rs): `mark_outcome_unknown`, `recover_unknown` | Reconciles that in-memory revision state. |
| [kernel/store/lifecycle.rs](src/kernel/store/lifecycle.rs): `install_purge_tombstone`, `apply_exact_object_deletion` | Applies old lifecycle state/receipt rules; not a connected general filesystem deletion service. |
| [kernel/backend.rs](src/kernel/backend.rs): `RevisionObjectBackend` | An encrypted-object trait. A declaration is not the missing persistent implementation. |
| [kernel/model.rs](src/kernel/model.rs): `EncryptedRevisionPayload::new` | Owns ciphertext and computes its actual BLAKE3 digest. Encryption is a requirement of this old model, not the new local baseline. |
| [legacy_inventory.rs](src/legacy_inventory.rs) and [submodules](src/legacy_inventory) | Classify legacy names, construct physical inventories, paginate and render migration reports. |

`RevisionStore` stores a `BTreeMap` of states plus operation, deletion and tombstone vectors.
`prepare_append` searches retained operation identities and checks their lifetime capacity.
There is no production `RevisionObjectBackend` implementation or production kernel caller connecting
this model to the new native ingestion path. Avoid creating a second persistent state machine to
make this otherwise unused model live.

## Actual daemon I/O

[secure_direct_store_storage_io.rs](../../../bins/eliot-searchd/src/secure_direct_store_storage_io.rs)
supplies `DaemonImmutableObjectPlatform`. Its `read_revision_object`,
`read_revision_object_detailed_with_context` and `persist_revision_object` call this package.

The private `publish` kernel writes/syncs a stage, reads it completely, links the final name, syncs the
directory and reads the final object completely. Existing-file reuse also reads and compares bytes.
Those are current mechanics. Keep the needed final durability/integrity boundary, while removing
duplicate full reads and a second implementation of the same mechanism.

The adjacent preparation adapter calls materializer's almost identical artifact engine. Use one
existing byte primitive for both callers; preparation still owns its payload schema. No per-kind
filesystem platform, receipt family or new storage framework is needed.

## Connected result to implement

1. Accept the actual acquired body and its source observation; do not reacquire a pathname.
2. Persist immutable bytes and validate the stored object once at the selected storage boundary.
3. Return the usable object reference and owned/borrowed verified bytes to the existing caller.
   The caller commits the revision/source head in redb after successful object publication.
4. Reopen by that retained revision reference, verify actual bytes and range, and feed preparation or
   evidence reads. A failed or missing read does not substitute the current file.

The useful record is revision identity, content digest/length and object locator. Keep real pending
effects where recovery needs them. Pure handoffs need no durable receipt, and completed local work
does not need another permanent replay archive.

Remove old encrypted/residency/lifecycle and inventory producers with their exclusive consumers.
A new binary can reject an unsupported old root before writes; no compatibility converter, export
platform or migration programme must precede this path. Leave existing originals and sole copies intact.
