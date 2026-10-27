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
→ create unique stage in final parent
→ write exact bytes while computing content identity
→ verify address/length before finalization
→ flush file data according to advertised durability profile
→ publish final name no-clobber through platform owner
→ if final exists, open through ordinary reader and verify exactly
→ verify final object once
→ remove this writer's stage where safe
→ return ObjectRef and optionally retained verified bytes
```

Two writers of identical bytes converge. Different bytes/profile/kind at one address are immutable conflict. A
known pre-publication failure removes the stage where safe. Possible publication resolves through exact final
readback. Leftover stages or unreferenced finals are stopped-root maintenance input, not operation history.

## Windows finalization — corrected R054 boundary

Git's loose-object flow remains the high-level donor. A random-stage helper may create the file, but no helper owns
final publication authority.

Rejected baseline operations:

- path-based `tempfile::persist_noclobber` / `MoveFileExW`;
- the reachable legacy relative-`RootDirectory` Win32 wrapper in source registration;
- private user-mode `NtSetInformationFile` or an equivalent direct NT call selected by R053.

Use the same documented finalization seam as the root owner:

```text
opened verified stage handle with DELETE access
+ retained verified directory-handle chain from data root to final parent
  (share read/write; no FILE_SHARE_DELETE)
+ canonical absolute content-addressed final path derived from retained final parent
→ SetFileInformationByHandle(FileRenameInfo)
   RootDirectory = NULL
   ReplaceIfExists = FALSE
→ reopen final relative to retained parent
→ compare stable file identity
→ verify format/profile/kind/address/length/content
```

The no-delete-share chain prevents replacement of any path component during the operation. Required canary cases:

- accepted absolute/extended path form and exact access/share flags;
- absent destination succeeds;
- existing identical destination converges only after ordinary-reader verification;
- existing conflicting destination refuses;
- ancestor/parent rename or replacement attempts fail while handles are retained;
- unsupported/non-local/cross-volume filesystems refuse;
- failed call leaves a classifiable stage/final state;
- exact relative reopen and identity/content readback;
- file-data flush, namespace visibility and sudden-power-loss durability remain separate claims.

Official docs describe relative `RootDirectory`, but a current supported-build report shows the wrapper may
absolutize the destination and fail with a non-null root handle. Do not use that form until demonstrated. Do not
work around it with private NT APIs.

If the documented absolute-target operation is unavailable for the advertised profile, refuse. Do not silently
use `MoveFileExW`, direct NT calls, hard-coded path-only helpers or a generic filesystem abstraction.

## Unix finalization

Prefer an owner-private directory-relative no-replace rename. A hard-link + unlink fallback is allowed only on the
same filesystem, with exact final readback and visible leftover-stage handling. `std::fs::rename` may overwrite and
is not selected.

## Reads

- never reacquire a mutable live source path;
- validate format/profile/kind/length before returning bytes;
- use the original request deadline, cancellation and byte counters;
- reject declared oversize before allocation/I/O;
- share one integrity pass across ranges from the same object;
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
metadata, real OS-protected key owner and authenticated envelope checks. Encrypted publication reuses the same
documented platform finalization seam and does not create another Windows rename or private-NT path.

## Removal boundary

Delete duplicate materializer/DIRECT writers, contextual receipt/stage APIs, in-memory object lifecycle histories,
legacy inventory/migration codecs and old DIRECT formats after current callers move. Move only proven useful
relative-open/native-identity code from the legacy daemon owner; delete its publication phases, wrapper and direct-NT
experiments.

## Focused checks

Plain profile: zero/nonzero bytes, concurrent identical writers, final conflict, stage replacement, ancestor/parent
replacement, destination-exists refusal, stage cleanup, restart reopen, corruption, range bounds, cancellation and
truthful durability status. Run minimal Rust 1.98 check/Clippy before native filesystem cases.
