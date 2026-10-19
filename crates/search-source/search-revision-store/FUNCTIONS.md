# search-revision-store — source and cutover map

## Current source

| Location / symbols | Actual behavior |
|---|---|
| `immutable_object.rs` | Bounded legacy object read and immutable stage/final publication through a supplied platform. |
| `immutable_object/context.rs` | Reimplements the same mechanics with request context, work accounting, attempted-effect phases and a receipt. The source file is about 40 KiB before tests. |
| `ContextualRevisionObjectReceipt::staging_cleanup_deferred` | Reports that this call did not clean its stage. Successful contextual publication commonly leaves the stage behind. |
| `LegacyRevisionObjectRead::into_bytes` | Transfers the owned object buffer without another full copy. Keep this ownership pattern. |
| `kernel/store/*` | In-memory Pending/Active/tombstone/operation vectors driven by supplied intents/readbacks. It is not the persistent object store. |
| `legacy_inventory*` | Old filename classification and migration inventory/reporting. |

The daemon `DaemonImmutableObjectPlatform` calls the immutable-object kernel. Publication currently writes
and syncs a stage, reads the full stage, links/publishes the final name, syncs the directory and reads the
full final object. Existing-object reuse also reads and compares bytes. Materializer has a second copy of
this platform with its own receipt family.

The platform durability claim is inconsistent: `sync_directory_result` calls `File::sync_all` on a Unix
directory, while the non-Unix `sync_directory` implementation is a no-op and the wrapper still returns
`Ok(())`. That may be sufficient for no durability guarantee, but it cannot be reported as an executed
Windows directory flush.

## Target API

Exact names may follow #598's accepted handoff, but one owner must provide the following behavior:

```text
put_object(kind, profile, owned bytes, OperationContext) -> ObjectRef
open_object(ObjectRef, OperationContext) -> VerifiedObject
read_ranges(ObjectRef, ranges, OperationContext) -> VerifiedRanges
list_objects(cursor, limit) -> bounded object identities
remove_exact(ObjectRef) -> known absent | removed | unresolved
```

`ObjectRef` binds the current-format address, kind/profile, content identity and exact length. It does not
contain source membership, current-head or query authority.

## Publication sequence

1. Validate admitted final directory, final address/kind/profile and byte ceilings.
2. Create one unpredictable `create_new` stage in the final directory. Retry a bounded name collision;
   do not share a fixed temporary name between concurrent writers.
3. Stream/write exact bytes while computing/verifying the selected content identity.
4. Flush/sync the stage according to the supported storage profile.
5. Publish no-clobber. If another writer already created the final name, open and verify it exactly.
6. Verify the resulting final object once.
7. Complete the platform-qualified namespace durability step.
8. Remove the stage after either verified reuse or verified publication.
9. Return `ObjectRef` and, where useful, the already verified owned bytes to the caller.

Two concurrent writes of the same bytes converge on one final object. A final name containing different
bytes is an immutable conflict. A failure before possible publication removes the stage when possible. A
failure after possible publication is resolved by the exact final address/readback. Stage cleanup remains
best effort and bounded; a leftover is GC input, not a successful-operation record.

## Donor code already present

`crates/search-control-redb/src/migration/cutover_artifact.rs` already implements the useful small sequence:

```text
create_new temporary
→ write_all + file.sync_all
→ temporary readback
→ fs::hard_link(temp, final)
→ remove temp
→ directory sync
→ final readback
```

Use this as an internal code donor while deleting the migration owner. Git's loose-object finalization
confirms the same temporary-to-content-addressed-final and temporary-removal pattern.

Do not copy its fixed temporary basename into a concurrent object store. Do not copy its over-conservative
rule that byte-identical readback after an unclassified native error can never resolve the desired final
state; object publication recovery is determined by whether the exact content-addressed final object now
exists and verifies, not by proving which process call created it.

## Windows finalization boundary

- `hard_link` requires same-volume filesystem support; creating the stage in the final directory handles
  the volume issue but not an unsupported filesystem.
- the current Windows directory sync is a no-op; exact final readback is not a power-loss durability test;
- select and qualify a supported native no-clobber/write-through move/link strategy for the supported
  Windows filesystem, or expose a weaker durability profile explicitly;
- test competing writers, destination already present, antivirus/file sharing, crash before/after final
  namespace change and restart before committing the redb source/preparation pointer;
- never silently fall back to overwrite, copy/delete or a receipt-only success.

The source/preparation pointer is committed only after the final object is known to satisfy the selected
storage profile. If that profile cannot establish crash durability, the application must report that fact
rather than claim a stronger guarantee.

## Range and integrity reads

- never reacquire the live pathname;
- reject foreign/corrupt kind/profile/length before returning evidence;
- enforce the request's original absolute deadline and cancellation;
- reserve input bytes before allocation/read;
- group selected ranges by object and merge same-basis overlaps;
- share a required full integrity pass across those ranges;
- return exact coordinates and scanned-byte accounting.

Do not represent a digest field returned beside arbitrary bytes as verification.

## GC boundary

The object store enumerates and deletes exact objects. Reachability belongs to current source heads,
current/pending view manifests and live view guards. Registering an object before or during creation may
help discover crash debris; it must not become a second permanent graph or operation journal.

Compute one bounded deletion batch from a coherent root snapshot. Recheck/delete only exact unreferenced
addresses. On Windows an open-handle/share-mode conflict may make deletion fail; keep the object visible for
a later pass. A failed delete is not evidence that the object is referenced and is not a reason for a
separate tombstone database.

## Remove after cutover

- contextual compatibility platform and duplicate materializer artifact platform;
- deferred-stage receipt/disposition APIs;
- in-memory append/recovery/purge operation histories with no current caller;
- legacy inventory/migration codecs after unsupported old roots are isolated;
- old encrypted/DIRECT object variants after their last declared reader moves.
