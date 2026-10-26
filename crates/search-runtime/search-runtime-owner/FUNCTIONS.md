# Root owner — one guard, manifest-last plain initialization

This owner classifies, initializes and opens one current-format data root. It owns the retained root handle and
one actual process-lifetime exclusive lock. It does not own source policy, object identity, Qdrant, query,
configuration activation or a generic recovery platform.

## Evidence boundary

Historical public-main inspection found more than 553 KiB of root/lifecycle Rust around real lock handles. Those
measurements and callers are revision-specific evidence, not a deletion quota. Recheck exact current manifests,
Rust/non-Rust callers, features, targets, tests and workflows during the code cut.

## Concrete seam

Names may vary:

```text
inspect_root(path) -> RootInspection
initialize_plain_root(path, settings, context) -> CurrentRootMetadata
open_current_root(path, context) -> OpenedDataRoot

OpenedDataRoot {
  retained canonical root handle
  one exclusive lock handle
  current root-format metadata
}
```

Root metadata contains only disk compatibility/identity needed to open concrete owners, for example:

```text
format version
root ID
storage profile = plain
control schema version
object schema version
```

It is not process history. #598 may later add an encrypted storage profile without changing the plain root into a
secret-lifecycle owner.

## Inspect — zero writes

Distinguish:

```text
Absent
Current(CurrentRootMetadata)
IncompleteInitialization
UnsupportedPreRelease
Corrupt
BusyOrOwned
ForeignOrUnsafePath
```

Inspection creates no directory, marker, lock, manifest, log, journal or repair state.

## Initialize — selected discard/retry model

Foundation creates only an empty current-format control/object layout. No user source, prepared artifact or
Qdrant authority exists yet. Therefore baseline recovery does not need replay/resume.

```text
validate selected local target and absence/recognized incomplete state
→ acquire real initialization exclusion
→ create exact owned initialization marker
→ create minimal control/object layout with create-new semantics
→ flush/close/verify the staged empty layout
→ create and sync final-manifest temp inside the root
→ publish final root manifest no-clobber last
→ reopen/verify manifest and exact layout
→ remove marker/stage debris
```

Prefer `tempfile::NamedTempFile::new_in(root)` plus `persist_noclobber` for final-manifest publication if it
preserves selected Windows path/no-reparse semantics. Otherwise use one small root-owner native finalizer. Do not
create a general filesystem layer.

A crash before final publication leaves `IncompleteInitialization`; a crash after successful publication leaves a
current root or an uncertainty resolved by inspection.

### Explicit discard only

`initialize --discard-incomplete` may remove/recreate only when all of the following hold under exclusion:

- no committed final root manifest exists;
- the exact current-format initialization marker is present and valid;
- only the recognized empty control/object layout and store-owned stages exist;
- no source/preparation/published/pending/import reference exists;
- every opened path/handle still identifies the object originally inspected.

Unknown files, foreign/replacement objects, conflicting bytes or deletion uncertainty refuse without mutation.
The user may relocate/remove the directory manually after inspection.

There is no baseline `--resume`, phase taxonomy, operation digest, original-input archive, owner epoch,
finish/abandon receipt or replay of partial effects.

## Open current root

```text
parse invocation/settings first
→ open/canonicalize selected local root without following replacement/reparse paths
→ retain root handle
→ acquire one exclusive current lock
→ reread/verify final manifest through retained handles
→ open typed redb/plain-object/source/publication owners
→ retain OpenedDataRoot/DataRootGuard in daemon composition
```

The guard may privately hold actual platform handles for locality, replacement and ACL checks. It exposes no
serializable authority. A second process fails `AlreadyOwned`; PID/executable records are not ownership.

## Shutdown

```text
stop admission
→ cancel/drain finite requests and QueryWindows
→ settle/stop publication and Qdrant users
→ close source/object/redb owners
→ drop DataRootGuard
```

A component reports its own failure. No `DRAINING/RELEASED` record or shutdown receipt reclassifies effects.
Process crash releases the OS lock; the next process validates current stores directly.

## Lock boundary

The guard spans process/store lifetime but is not acquired per request. No redb writer, view-manager mutex or
QueryWindow mutation borrow may be held while acquiring the guard or doing root/object/Qdrant/process I/O.

## Remove with callers

Remove supervisor prepare/complete/recover/release transitions; process/executable identities not used by the
actual lock; leases/health/drain/dependency receipts; dual compatibility locks; owner slots/succession;
`NativeRootRequest` argv/stage custody; initialization journal; and root-level source/registry/DEK/migration/
service wrappers.

Keep actual locality/no-follow/no-reparse/ACL checks, retained handles, current manifest classification, bounded
context and content-free errors.

Stopped-root object collection/archive/import acquire this same guard only after daemon/store shutdown. Online
Qdrant retirement belongs to publication.

## Focused evidence after code

- zero-write inspection for every classification;
- explicit empty initialization and final-manifest-last readback;
- crash/failure before and after every write/finalization point;
- discard exact incomplete layout versus foreign/replacement refusal;
- second-process exclusion and lock release after crash;
- unsupported/pre-release root unchanged;
- store-open failure closes partial owners and releases guard without repair;
- truthful Windows durability profile and shutdown order.

Run minimal Rust 1.98 check/Clippy for root owner and actual daemon/store consumers before focused native cases.