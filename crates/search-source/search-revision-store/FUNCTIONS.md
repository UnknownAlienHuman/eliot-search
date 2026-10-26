# Immutable object store — plain baseline, optional encrypted profile

This owner persists source bodies and prepared artifacts. It decides object identity, publication, verification,
range reads and exact deletion mechanics. Source membership, preparation currentness, publication reachability and
query scope belong elsewhere.

## Baseline boundary

Implement the `Plain` current-format profile first. It has no key, secret catalogue or encryption lifecycle.
#598 may later provide an encrypted profile through the same Eliot-owned `ObjectRef` after an accepted public
handoff. It must not create a second source/preparation/evidence path.

```text
put_object(kind, Plain, owned_bytes, RequestContext) -> ObjectRef
open_object(ObjectRef, RequestContext) -> VerifiedObject
read_ranges(ObjectRef, ranges, RequestContext) -> VerifiedRanges
list_objects(cursor, limit) -> bounded identities
remove_exact(ObjectRef) -> Absent | Removed | Unresolved
```

`ObjectRef` binds current object-format version, storage profile, kind, content address and exact length. It carries
no source membership, current-head, access or query authority.

## Plain publication

```text
validate final namespace/address and byte limit
→ create unique stage in the final parent
→ write exact bytes while computing content identity
→ verify address/length before finalization
→ flush file data according to the supported durability profile
→ publish final name no-clobber through the platform owner
→ when final exists, open through the ordinary reader and verify exactly
→ verify final object once
→ remove this writer's stage when possible
→ return ObjectRef and optionally retained verified bytes
```

Two writers of the same bytes converge. Different bytes/profile/kind at one address are immutable conflict. A
known failure before publication removes the stage where safe. Possible publication resolves through exact final
readback. Leftover stages or unreferenced finals are stopped-root maintenance input, not operation history.

## Platform primitive correction

Git's loose-object flow remains the high-level donor. `tempfile 3.27.0` may help create a random same-directory
stage, but its current Windows `persist_noclobber` implementation is path-based `MoveFileExW` after a path-based
attribute change. It is not the selected trusted finalizer.

### Windows

Use one private handle-based primitive shared only with the root manifest owner:

- create/open stage with the rights and share flags required for final rename and verification;
- retain the verified final-parent/root handle;
- rename the opened stage handle to a relative final leaf with replacement disabled;
- reopen the final through the retained directory/root and verify kind/profile/address/length/content;
- refuse the platform profile if the tested handle-relative operation is unavailable.

Do not fall back silently to path-only rename. Execute the `SetFileInformationByHandle` canary before production
implementation. File-data flush and namespace publication are separate claims; final readback is not sudden-
power-loss proof.

### Unix

Prefer an owner-private directory-relative no-replace rename. A hard-link + unlink fallback is allowed only on the
same filesystem, with exact final readback and visible leftover-stage handling. `std::fs::rename` may overwrite and
is not selected.

## Reads

- never reacquire a mutable live source path;
- validate format/profile/kind/length before returning bytes;
- use the original request deadline, cancellation and byte counters;
- reject declared oversize before allocation/I/O;
- share one required integrity pass across ranges from the same object;
- return exact coordinates and scanned-byte accounting;
- corruption, wrong profile and short/long content are hard failures.

A digest returned beside arbitrary bytes is not verification.

## Metadata commit

Object publication/readback completes outside the short redb source/preparation transaction. A stale conditional
commit may leave an unreferenced immutable object for stopped-root maintenance but cannot overwrite current state.
No redb writer spans object I/O.

## Reachability and deletion

The store enumerates/removes exact objects; it does not decide reachability. Stopped-root maintenance marks from
current source/preparation rows, published/pending state and retained archives, then sweeps exact unreferenced
objects. Failed Windows deletion remains visible for another run.

No online object GC, tombstone database, purge proof or completed-operation archive belongs to the baseline.

## Encrypted profile handoff

After #598 publishes an accepted implementation, retain only its concrete encrypted `put/open/read`, exact profile
metadata, real OS-protected key owner and authenticated envelope checks. Plain and encrypted profiles may share
small object format/address helpers. Do not restore a universal secret catalogue, provider registry, rotation/
migration platform or legacy six-domain protocol.

## Remove with callers

Delete duplicate materializer/DIRECT writers, contextual receipt/stage APIs, in-memory object lifecycle histories,
legacy inventory/migration codecs and old DIRECT formats after exact current callers move.

## Focused checks

Plain profile: zero/nonzero bytes, concurrent identical writers, final conflict, stage replacement, parent
replacement, stage cleanup, restart reopen, corruption, range bounds, cancellation and truthful durability status.
Run minimal Rust 1.98 check/Clippy for store and immediate consumers before native filesystem cases.
