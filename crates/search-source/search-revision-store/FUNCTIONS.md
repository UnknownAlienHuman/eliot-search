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

1. Validate final address/kind/profile and byte ceilings.
2. Create one unpredictable stage in the final directory.
3. Stream/write exact bytes while computing/verifying the selected content identity.
4. Flush/sync the stage according to the supported storage profile.
5. Publish no-clobber. If the final name already exists, open and verify it exactly.
6. Verify the resulting final object once.
7. Remove the stage after either verified reuse or verified publication.
8. Return `ObjectRef` and, where useful, the already verified owned bytes to the caller.

A failure before possible publication removes the stage when possible. A failure after possible
publication is resolved by the exact final address/readback. Stage cleanup remains best effort and
bounded; a leftover is GC input, not a successful-operation record.

The useful donor pattern is Git's loose-object finalization: publish a temporary object to its content
address, then remove the temporary name after successful link/rename. Eliot still supplies its own current
format, integrity and platform durability requirements.

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
help discover crash debris; it must not become a second permanent graph or operation journal. Failed delete
leaves the object discoverable for a later pass.

## Remove after cutover

- contextual compatibility platform and duplicate materializer artifact platform;
- deferred-stage receipt/disposition APIs;
- in-memory append/recovery/purge operation histories with no current caller;
- legacy inventory/migration codecs after unsupported old roots are isolated;
- old encrypted/DIRECT object variants after their last declared reader moves.
