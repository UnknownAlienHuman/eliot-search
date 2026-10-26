# Immutable object store — plain baseline, optional encrypted profile

This owner persists source bodies and prepared artifacts. It decides object identity, no-clobber publication,
verification, range reads and exact deletion mechanics. Source membership, preparation currentness, publication
reachability and query scope belong elsewhere.

## Baseline is independent of #598

Implement `Plain` first. It has no key, secret catalogue or encryption lifecycle. #598 may later supply an
encrypted profile through the same Eliot-owned `ObjectRef`; it cannot create a second source/preparation/evidence
path or block the plain product.

## Required boundary

Exact names may differ:

```text
put_object(kind, Plain, owned_bytes, RequestContext) -> ObjectRef
open_object(ObjectRef, RequestContext) -> VerifiedObject
read_ranges(ObjectRef, ranges, RequestContext) -> VerifiedRanges
list_objects(cursor, limit) -> bounded identities
remove_exact(ObjectRef) -> Absent | Removed | Unresolved
```

`ObjectRef` binds current format, storage profile, kind, content address and exact length. It carries no source
membership, current-head, access or query authority.

<a id="plain-publication"></a>
## Plain publication — selected small implementation

The preferred stable-Rust candidate is `tempfile::NamedTempFile` in the final object's directory. R048 inspected
`tempfile 3.27.0` (MSRV 1.63); pin/recheck it only with the implementation.

```text
validate object kind/profile and input ceiling
→ determine final parent/address namespace
→ NamedTempFile::new_in(final_parent)
→ write exact bytes while streaming the content address
→ verify computed address/length
→ sync_all according to the advertised durability profile
→ persist_noclobber(final_path)
→ if AlreadyExists: verify the existing final object exactly
→ open/verify the final object once
→ return ObjectRef, optionally retaining verified owned bytes
```

This borrows Git's useful loose-object pattern: same-directory stage, streaming identity, file sync and
content-addressed finalization. Do not copy Git's skipped collision check. Different bytes/format/profile/kind at
one address are a hard immutable conflict.

`std::fs::rename` is not selected because it may replace the destination. Stable Rust 1.98 has no stable
`rename_noreplace`. `persist_noclobber` may leave a stage or second hard link after crash/unlink failure; that is
safe visible maintenance debris, not a durable operation history.

If the selected Windows retained-handle/no-reparse rules cannot be preserved by the path API, implement one tiny
native no-clobber finalizer inside this owner. It must keep the same operation contract and must not become a
generic filesystem framework.

Known pre-publication failure removes this writer's stage when possible. Possible publication is resolved only by
exact final-address readback.

## Durability statement

Expose what was executed, for example:

```text
NamespaceAtomicAndFileSynced
NamespaceAtomicReadbackOnly
```

Do not claim sudden-power-loss durability from namespace readback, `File::sync_all` alone or a no-op directory
sync. A stronger Windows profile requires platform execution. The baseline may ship a truthful weaker profile
rather than block product integration on an unproven claim.

## Reads

- never reacquire a mutable live source path;
- validate format/profile/kind/address/length before returning bytes;
- use the original deadline, cancellation and byte counters;
- reject declared oversize before allocation/I/O;
- share one required integrity pass across ranges from the same object;
- return exact coordinates and scanned-byte accounting;
- corruption, wrong profile and short/long content are hard failures.

A digest returned beside arbitrary bytes is not verification. For the first walking skeleton a full verified read
is acceptable; optimize range verification only from measurement, not by inventing a Merkle framework.

## Metadata commit boundary

Object I/O completes before the short source/preparation redb transaction. The transaction installs the verified
`ObjectRef` only if its captured non-reset guard still matches. A stale/conflicting transaction may leave an
unreferenced immutable object; stopped-root maintenance can reclaim it later. It must never roll back or delete a
newer/current object.

## Reachability and deletion

The store enumerates and removes exact objects but does not decide reachability. Stopped-root maintenance marks
from current source/preparation state, published/pending state and incomplete import, then sweeps exact recognized
unreferenced objects.

No online object GC, tombstone database, reference counter, purge proof or completed-operation archive.
Failed Windows deletion remains visible for another maintenance run.

## Encrypted handoff

After an accepted #598 implementation, retain only its concrete encrypted `put/open/read`, exact `ObjectRef`
profile metadata, authenticated envelope checks and real OS-protected key owner. Plain and encrypted profiles may
share small address/format helpers; do not restore a universal secret catalogue, rotation/migration system or
second store.

## Remove with callers

Delete duplicate materializer/DIRECT writers, contextual receipt/stage APIs, in-memory lifecycle histories,
legacy inventory/migration codecs and compatibility readers after exact current callers move.

## Focused evidence after code

- zero/nonzero and maximum accepted bytes;
- concurrent identical writers;
- final existing identical versus conflicting/corrupt;
- streaming address/length mismatch;
- stage/hard-link debris after interruption;
- persisted reopen and range bounds;
- cancellation before/during write and before finalization;
- exact advertised durability on selected Windows profile;
- stale metadata commit leaves safe orphan only.

Run minimal Rust 1.98 check/Clippy for the store and immediate source/preparation/evidence consumers before these
focused cases.