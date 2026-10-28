# Immutable object store

One owner persists source bodies and prepared artifacts. It owns content identity, publication, verification,
ranges and exact deletion mechanics; source membership, currentness, query scope and reachability belong elsewhere.

## Plain baseline and API

Implement plain current-format storage without keys. An accepted #598 encrypted profile may later use the same
ObjectRef and source/preparation/evidence path; do not inspect or compete with its private work.

```text
put_object(kind, Plain, owned_bytes, ExecutionContext) -> ObjectRef
open_object(ObjectRef, ExecutionContext) -> VerifiedObject
read_ranges(ObjectRef, ranges, ExecutionContext) -> VerifiedRanges
list_objects(cursor, limit) -> bounded identities
remove_exact(ObjectRef) -> Absent | Removed | Unresolved
```

ObjectRef binds format, storage profile, kind, content address and exact length, not membership or access authority.
[ExecutionContext](../../../docs/implementation/README.md#minimal-cross-owner-values) supplies the calling operation's
deadline/cancellation and actual byte limits. It never requires a PublishedView or query-scoring profile. Queries
pass their existing execution budget through the reader without resetting it.

## Immutable publication

```text
validate namespace/address/limits → unique stage in final parent
→ write exact acquired bytes while computing identity → check address/length → file-data flush
→ shared platform no-clobber finalizer → exact existing/final verification → safe stage cleanup → ObjectRef
```

Do not reread a mutable source pathname. The [root owner's platform recipe](../../search-runtime/search-runtime-owner/FUNCTIONS.md#windows-finalization)
owns the Windows operation and canary; its Unix section owns that platform's no-replace rule. Root and object code
share a small private mechanism, not duplicated wrappers or a filesystem framework. No new platform guarantee is
established by this page.

Identical writers converge only after ordinary-reader verification of the existing final. Different bytes,
profile or kind at one address are an immutable conflict. Never compare the current writer's stage identity to an
independently published identical object as though both must be the same file.

Known pre-publication failure cleans owned stages where safe. Possible publication resolves through exact final
readback. Uncertain outcomes preserve classifiable debris rather than overwrite or report success. Stages and
unreferenced finals are stopped-root maintenance input, not permanent operation history.

## Reads

Validate format/profile/kind/declared length before allocation and enforce actual byte ceilings while reading.
Use the original deadline/cancellation. Verify content integrity and returned ranges; a supplied digest alongside
arbitrary bytes is not verification. Short/long content, corruption, wrong profile or invalid ranges fail.

Share one integrity pass across ranges of the same object where useful and charge all bytes actually read/hash-
verified, not only excerpt length. A small output page does not make whole-object integrity verification cheap.
Never reopen the live source to substitute missing retained evidence.

## Metadata and lifetime

Complete object publication/readback before the short conditional source/preparation redb commit. No redb writer
spans object I/O. Stale work can leave an unreferenced immutable object but cannot overwrite a newer source head.

The store enumerates/removes exact objects; it does not decide reachability. Stopped-root maintenance marks the
current/prepared/published/pending/cleanup/archive references under exclusive ownership, then sweeps only proven
unreferenced objects. Uncertain traversal/deletion preserves bytes. No online object GC or purge-proof platform.

## Optional encryption and removal

After accepted #598 handoff, retain only its concrete codec, key-use seam and exact authenticated profile checks.
Encrypted publication uses the same platform finalizer. Plain storage has no secret lifecycle.

Remove duplicate materializer/DIRECT writers, stage/receipt APIs, in-memory lifecycle histories, obsolete inventory/
migration codecs and old formats with callers. Retain a narrow old-data reader only when explicit user-data access
requires it; unsupported old roots otherwise remain untouched.

## Checks after code

Run minimal locked Rust 1.98 check/Clippy for the store and actual source/preparation/query consumers. Focused
cases: empty/nonempty input, identical writers, conflicting final, stage/path replacement, failed finalization,
restart, corruption/range bounds, pre-I/O oversize rejection, active cancellation and honest durability.
