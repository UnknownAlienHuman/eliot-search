# Immutable object store

One plain writer/reader owns retained source and prepared bytes. [Local scope](../../../docs/architecture/LOCAL_SCOPE.md)
excludes application encryption; source membership and reachability belong to their actual owners.

## Plain baseline and API

```text
put_object(kind, owned_bytes, ExecutionContext) -> ObjectRef
open_object(ObjectRef, ExecutionContext) -> VerifiedObject
read_ranges(ObjectRef, ranges, ExecutionContext) -> VerifiedRanges
list_objects(cursor, limit) -> bounded identities
remove_exact(ObjectRef) -> Absent | Removed | Unresolved
```

ObjectRef binds format version, kind, content address and exact length. No cipher, key or residency/security-profile
field is needed. Use the caller's deadline/cancellation and actual byte allowance; no query view or scoring state.

## Immutable publication

```text
exact acquired bytes → bounded hash/length check
→ private same-parent stage → write/sync → no-clobber finalization
→ ordinary-reader verification of existing/final object → safe stage cleanup → ObjectRef
```

[Root's shared helper](../../search-runtime/search-runtime-owner/FUNCTIONS.md#shared-no-clobber-publication)
owns file publication on both platforms. Do not duplicate its implementation or introduce a filesystem framework.
Do not reopen a mutable source pathname. Identical writers converge only after checking exact kind/length/content;
independently published identical objects need not share the stage's physical file identity. A conflicting final
never gets overwritten. Resolve uncertainty by exact readback; keep classifiable debris instead of inventing success.

## Reads

Validate format/kind/declared length before allocation and enforce actual limits while reading. Verify the bytes
and requested ranges, not echoed metadata. Short/long content, wrong address, corruption and invalid ranges fail.
A missing retained object is not permission to reopen a live source as substitute evidence.

Share one integrity pass across selected ranges when useful and charge all bytes read/hashed, not only excerpt
length. Keep verified buffers or the selected reader's actual integrity guarantee; do not verify one file and then
silently reopen an unrelated one. No encryption pages, signatures or verification receipts are required.

## Metadata and lifetime

Finish object publication/readback before the short guarded source/preparation redb commit. No writer transaction
spans object I/O. Stale work may leave an unreferenced object, never replace a newer source head.
The store removes exact objects but does not decide reachability. [Stopped-root maintenance](../../search-runtime/search-retention/FUNCTIONS.md#stopped-root-local-object-collection)
marks current/prepared/published/pending/cleanup/import references first. No online local-object GC.
Writer-owned temporary cleanup is separate from deciding that final user data is unreferenced.

<a id="optional-encryption-and-removal"></a>
## Legacy removal

Remove duplicate DIRECT/materializer writers, key/profile selectors, receipts and lifecycle histories with their
actual consumers. Preserve unpublished #598 work; it is not an integration requirement. Unsupported encrypted/old
roots remain untouched. Do not create new keys or silently decrypt/relabel them; a specific user-data export is separate.

## Checks after code

Minimal locked check/Clippy for store and real source/preparation/query consumers. Later checks: empty/nonempty
round trip, identical/conflicting publication, interruption, corrupt/missing bytes, ranges, finite memory/read
cost, cancellation and factual durability. The plain implementation and these runs are not supplied by this page.
