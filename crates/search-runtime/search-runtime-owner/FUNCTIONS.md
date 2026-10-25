# search-runtime-owner — source/caller audit and replacement map

## Evidence boundary

At original public-main baseline `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`, the pure package contained
123,087 bytes of production Rust and daemon `owner_composition/kernel*` contained 443,814 bytes before tests.
The audit found package-local/test construction for the generic supervisor/transition API and one real daemon
caller for root-open classification. These measurements and caller findings are historical evidence at that
revision, not current branch truth.

At the implementation cut, search current workspace/package manifests, Rust imports/constructors/path inclusion,
non-Rust tooling, features, targets, tests and workflows. Record every surviving consumer and move it in the same
commit before deleting an API or crate boundary.

## Current implementation defects represented by the inspected baseline

The inspected code combined real exclusion with a second logical operating system:

- `NativeRootRequest` retained argv, request digest, six work-meter dimensions, named stage arrivals and
  command-specific source/key/registry/migration parsers;
- open acquired legacy and sealed lock files, then consulted alternating ACTIVE/DRAINING/RELEASED owner slots;
- owner records carried process/executable identity, epochs, generations and succession although the retained
  locked handles already supplied exclusion;
- initialization used a journal/state taxonomy instead of one staged current-format publication;
- shutdown generated dependency/drain/release records instead of closing concrete owners in order.

Do not preserve these mechanisms merely because old tests or forwarding wrappers still mention them.

## Target concrete seam

Suggested names are not mandatory:

```text
inspect_root(path) -> RootInspection
initialize_root(path, settings, context) -> CurrentRootMetadata
open_current_root(path, context) -> OpenedDataRoot

OpenedDataRoot {
  retained canonical root handle
  exclusive lock handle
  current root-format metadata
}
```

The daemon owns `OpenedDataRoot`. Concrete control/source/object/publication owners open under it and close before
it. Root metadata contains only disk compatibility and identity required to open current stores, such as format
version, root ID, storage profile and schema versions. It is not process history.

## Required behavior

### Inspect

No writes. Distinguish absent, current, incomplete initialization, unsupported pre-release, corrupt, busy/owned
and foreign/unsafe path.

### Initialize

Use a unique owned staging area, build and verify stores, publish one current manifest/namespace last without
clobber, verify final state, then remove bounded staging debris. Normal open does not repair staging. Explicit
resume/discard acts only on an exact verified uncommitted staging instance.

### Open

Parse invocation/settings first. Retain the root handle, acquire one exclusive process-lifetime lock, reread
current metadata through retained handles, then open concrete stores. Keep selected real Windows/locality/
no-follow/no-reparse checks. Do not add PID liveness, executable hashes, owner epochs or a second lock as fallback
authority.

### Shutdown

Stop admission, drain/cancel finite work, release query windows/views, resolve concrete publication effects,
close concrete stores, then drop the root guard. A component reports its own failure; no generic owner receipt
reclassifies it.

## Removal order

1. Implement side-effect-free inspection and #360 manifest-last initialization.
2. Open current roots with one retained guard and small current metadata.
3. Open actual settings/control/source/object/publication owners under that guard.
4. Move current service/query/admin callers from command-shaped root capabilities to direct owners.
5. Delete argv/request digest, named stage/work meters and command-specific custody from root ownership.
6. Delete owner slots, succession, lifecycle records, dual locks and initialization journal.
7. Delete the supervisor/lease/transition API and move or inline the retained classifier/errors.
8. Delete the package dependency if no independent effect boundary remains.

Stopped-root local-object collection and archive/import use the actual guard only after the daemon is stopped;
they are specified in [`../search-retention/FUNCTIONS.md`](../search-retention/FUNCTIONS.md). Publication owns
online Qdrant retirement.

## Failure semantics

- invalid, unsupported, corrupt or foreign path before lock: known refusal and no writes;
- lock held: `AlreadyOwned`;
- initialization failure before final publication: known incomplete staging;
- uncertain final publication acknowledgement: explicit inspection decides current versus incomplete;
- current-store open failure: close partial owners and drop the lock without mutating the root;
- process crash: OS releases the lock and the next process validates current stores directly.

## Focused verification after code

Run minimal Rust 1.98 check/Clippy for changed owner and direct consumers. Then exercise zero-write inspection,
initialization crash points, second-process exclusion, restart, path replacement/reparse refusal, store open/close
ordering and shutdown. Historical lifecycle/receipt matrices are removed with the APIs and are not release gates.
