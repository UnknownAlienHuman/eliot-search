# Root owner — one local lock and plain format

[Local scope](../../../docs/architecture/LOCAL_SCOPE.md) selects a private per-user application directory,
not a sandbox against malicious same-user code. This owner classifies/initializes/opens the root and retains
one process lock. It owns no keys, source catalogue, query state or lifecycle ledger. This is target behavior.

## Public operations

```text
inspect(path) -> RootInspection
initialize_plain(path, settings, ExecutionContext) -> DataRootGuard
open_existing(path, settings, ExecutionContext) -> DataRootGuard
explicit_discard_incomplete(path, ExecutionContext)
```

Use execution-only deadline/cancellation and bounds, never a query view. Help/version have no root effects;
inspect is read-only. Do not add a plain/encrypted selection or generic storage-profile registry.

## Classification

| Observation | Result |
|---|---|
| Absent/empty selected directory | Uninitialized; only explicit init creates application files |
| Only the fixed lock file, or recognized unfinished empty layout with init marker; no final manifest | IncompleteInitialization; explicit init/discard only |
| Compatible final manifest and required stores, no incomplete-import marker | Current |
| Complete/partial archive layout | Not a runnable root |
| Incomplete-import marker | Normal open refuses before recovery/backend effects |
| Missing/conflicting committed files | Corrupt; no automatic repair |
| Unknown/unsupported layout, including old encrypted roots | Unsupported; leave data/keys intact |
| Another cooperating process holds the lock | Busy |

Check the actual selected directory and reject unsupported links/reparse entries in application state. Use
ordinary OS permissions; a root under a shared writable location is not the selected deployment. No arbitrary
parent-chain/anti-administrator security promise is made. Sources cannot supply application output paths.

## One lock

Use one stable lock file and [std::fs::File::try_lock](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock),
opened read/write without truncation. Existing-root open uses its existing lock; explicit initialization creates
it. Keep the handle in DataRootGuard until all stores/tasks close. WouldBlock means Busy; other errors are errors.
Do not reacquire through cloned handles, infer ownership from file existence/PID or steal a stale marker.
Never unlink/replace the lock file on shutdown: otherwise another opener can lock a different file at the same path.
All application writers and stopped-root maintenance cooperate with this lock. It is not a barrier to hostile OS users.
A crash before the init marker may leave only this lock file. Explicit init may continue from that recognized
otherwise-empty state after acquiring the same file; reclassify under the lock before any store writes.

## Final-manifest-last initialization

Validate the selected empty/recognized layout; take exclusion; write the owned init marker; create and verify
empty stores; publish the final manifest last through the helper below. Admit no user content before that manifest.
A failure before publication allows only strict recognized-empty discard/retry. No init journal or resume phases.
An uncertain publication is resolved from the exact final manifest/layout, never by overwriting it.

<a id="windows-finalization"></a>
<a id="windows-finalization--corrected-r054-boundary"></a>
<a id="unix-finalization"></a>
## Shared no-clobber publication

Use the maintained [tempfile NamedTempFile API](https://docs.rs/tempfile/3.27.0/tempfile/struct.NamedTempFile.html#method.persist_noclobber)
in the private final parent, on the same filesystem:

```text
NamedTempFile::new_in(final_parent)
→ bounded write of exact bytes; verify length/hash; as_file().sync_all()
→ persist_noclobber(final_path)
→ verify the final through the ordinary reader
→ remove owned temporary debris when safe
```

Pin the dependency with its real consumer at the code cut; the 3.27.0 API was inspected, not compiled here.
Keep one small private helper shared by manifest/object writers, not a new filesystem service. Existing final
means verify exact expected content; never fall back to overwriting persist/rename. A failed/uncertain outcome
retains enough owned debris for exact inspection and later stopped-root cleanup.

This supersedes the custom FileRenameInfo/RootDirectory/ancestor-handle design for the selected local threat model.
The library documents no-overwrite behavior, not universal atomic cleanup or power-loss durability; a leftover
stage link can remain. Do not use a public temp directory. Flush file data, sync directories where supported,
and report actual platform/filesystem durability. Success/readback is not proof against sudden power loss.
Do not add private NT calls or a second platform algorithm solely to recreate the retired hostile-writer model.

## Explicit discard

Require the exclusive guard, no final manifest, exact owned init marker, only recognized empty stores/stages and
no source/prepared/published/pending/import references. Unknown entries or uncertain identity/deletion refuse.
A committed or populated root is never eligible. Keep the stable lock file through discard/retry; do not unlink
it while another opener may hold it. No cleanup of originals or user keys.

<a id="archive-and-import-boundary"></a>
## Archive and import boundary

[Maintenance](../search-retention/FUNCTIONS.md#backup-is-a-completed-archive-not-a-runnable-clone) owns the separate
non-runnable archive layout. Ordinary open refuses archives and incomplete imports before backend recovery.
When Stage 5 import arrives, persist one destination-bound incomplete marker before data/RPCs; explicit import
retry acquires the same lock and validates that marker. Remove it durably only after verified complete destination
current/evidence state. Uncertain removal is resolved by exact readback. This is not an init-resume platform or
a Foundation prerequisite; add the marker/checks with the importing consumer.

## Open and shutdown

Validate existing root, acquire its lock, reread/classify metadata, then open stores. Unsupported roots are not
mutated to create missing files. Shutdown stops admission, cancels/drains work, drops windows/views, stops backend
users, closes stores and finally drops the guard. No DRAINING/RELEASED record or per-request root mutex.

## Removal and checks

Replace owner slots/epochs, leases, argv custody, key selection, migration wrappers and custom publication with
actual callers. Preserve data and unpublished work. After code run minimal locked check/Clippy for root/daemon/
store consumers. Later cases cover init interruption, same-file exclusion, no-clobber conflict, corrupt/unknown
roots, safe discard, archive/import refusal and restart. Record exact OS/filesystem and limits; no run is claimed.
