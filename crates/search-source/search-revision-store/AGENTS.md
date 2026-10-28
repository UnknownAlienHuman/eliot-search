# Working on search-revision-store

Read the root [architecture](../../../docs/architecture/README.md) and
[implementation](../../../docs/implementation/README.md). This package has a concrete byte-storage role;
it is not a separate lifecycle or transaction platform.

## Change the real path

- Trace acquired bytes through the daemon writer, revision reference and source-head transaction.
  Change the owner and its real consumers together.
- Start with [immutable object I/O](src/immutable_object.rs). Keep one byte writer/reader; consolidate
  the duplicate materializer artifact mechanics instead of adding another crate, port or forwarding layer.
- Keep source identity/head semantics in the existing source/control owner. Do not install the
  [in-memory kernel](src/kernel/store.rs) as a second catalogue or permanent operation ledger.
- Verify real content length/digest at the storage boundary and preserve that verified buffer through
  the handoff. Expected metadata returned beside arbitrary bytes is not a verified read.
- Keep immutable publication, durability and conflict handling. A successful known rollback is a
  failure/cancellation; an actual possible write needs readback or pending recovery.
- Clean up staging owned by this writer when its outcome is known. The old contextual APIs' permanent
  deferred-cleanup disposition is current inventory, not a rule for the new writer.

## Remove obsolete obligations

New local storage does not require six residency domains, encryption, copy/re-encrypt receipts,
retention-lease graphs or another completed-operation archive. Remove obsolete fields and exclusive
callers together; do not fabricate values to appease them.

Old names, layouts, error strings and fixture bytes are not compatibility commitments. Reject an
unsupported old root before writes and leave it intact. Do not delete user originals or sole retained
copies to simplify the implementation.

## Check the changed behavior

Exercise actual write/reopen, existing-object conflict, crash before/after the reference transaction
and cleanup of owned staging where changed. Check that the same retained bytes reach preparation and
evidence reads. Compile the affected production owner and consumers; documentation edits need only
documentation checks. No source-line quota, slice exception, migration issue or qualification receipt
is a prerequisite.
