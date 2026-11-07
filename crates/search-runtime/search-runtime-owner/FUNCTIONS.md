# Plain root owner

[Local scope](../../../docs/architecture/LOCAL_SCOPE.md) selects plain local files for one OS account.
This owner classifies, initializes, opens and retains the process lock. It owns no keys, source catalogue,
query state or lifecycle ledger.

## Public operations

    inspect_plain(&Path, Instant, &AtomicBool)
        -> Result<PlainRootInspection, PlainRootError>
    initialize_plain(&Path, Instant, &AtomicBool)
        -> Result<PlainRootGuard, PlainRootError>
    open_plain(&Path, Instant, &AtomicBool)
        -> Result<PlainRootGuard, PlainRootError>
    open_plain_with_recovery(&Path, Instant, &Arc<AtomicBool>)
        -> Result<PlainRootGuard, PlainRootError>
    discard_incomplete_plain(&Path, Instant, &AtomicBool)
        -> Result<(), PlainRootError>

Each call receives the caller's original absolute deadline and cancellation flag. The private operation context
passes those same values to every helper and to search-control-redb. There is no public context, deadline reset,
key, profile, settings or external initialization identifier.

PlainRootInspection exposes its path and status, with is_current() and is_incomplete().
PlainRootGuard retains the selected path and the open lock file; it exposes path(), object_dir(),
control_path() and is_current(). The error codes are bounded static strings. Errors distinguish
cancellation, deadline, filesystem I/O, unavailable control storage, bounded probe limit, lock contention,
unsupported format and corrupt committed state.

## Layout and classification

The root layout is root.lock, root.initializing, root.toml, objects/, and control/control.redb.
The only owned temporary basenames are root.initializing.stage and root.toml.stage, in that same root.
The committed manifest bytes are
format = "eliot-plain-root"\nversion = 1\n. The owned init marker is
eliot-plain-root-initializing-v1\n.

| Observation | Status |
|---|---|
| Missing or empty selected directory | Absent or Empty |
| Only a regular root.lock, its recognized partial marker stage, or exact marker plus recognized empty stores/stages and no manifest | IncompleteInitialization |
| Exact manifest, required layout, stable lock file, readonly control format and any exact completed metadata stages | Current |
| Missing committed files, damaged recognized metadata, or nonempty stores before manifest publication | Corrupt |
| Unknown entries, invalid marker, symlinks or unsupported/legacy/encrypted format | Unsupported |
| Another process holds the root or control lock | Busy |
| Reserved archive.complete entry | Archive |
| Reserved root.importing entry | IncompleteImport |

Unknown or unsupported roots are classified before initialization creates a lock or other file. Existing roots
are never repaired by inspect_plain or ordinary open_plain. The separate writable-open capability is described
below. Current validates the manifest and calls inspect_plain_control; it does
not infer a valid database from file presence and does not require an active control database to be
semantically empty.

Root enumeration is capped at 32 entries and control-directory enumeration at 16. Current-root validation
checks that objects/ is a directory but does not enumerate its retained contents. During incomplete-state
validation, it reads at most one object-directory entry to establish emptiness. There is no second-level
corpus or symlink scan.

The root owner uses search-control-redb for control initialization and readonly validation. Fresh control
metadata uses its redb 4.5.0 dependency at the same control/control.redb path that the control owner later
opens. inspect_plain_control checks only format metadata and version; Current does not enumerate tables, probe
emptiness or impose a database-size ceiling. For unfinished initialization, the root first holds root.lock
and then calls plain_control_is_empty directly, without first requiring the ordinary format marker. That bounded
probe uses the same opened file for its 64 MiB byte ceiling and accepts both vendor preformat emptiness
(zero regular tables and zero multimap tables) and the initialized format's sole metadata row. Extra tables or
metadata report nonempty; hitting the ceiling returns the distinct ProbeLimit error. Zero-length staging is
accepted as empty. Only explicit initialization under the lock and exact init marker may complete the control
format on a proven empty preformat database. Current still requires the format metadata. This module never
parses or copies database bytes; the control owner owns the guarded recovery copy and maintained redb operations.

## Stable lock and initialization

Use one stable root.lock, opened read/write without truncation and locked through std::fs::File::try_lock.
Keep the file handle in PlainRootGuard until all stores and tasks close. The lock file is never replaced or
unlinked. File existence, PID and lock-file contents do not establish ownership. This is cooperating-process
exclusion in the selected private application directory, not a sandbox against malicious code running as the
same account.

Explicit initialization accepts only an absent/empty directory or a recognized empty incomplete layout. It
creates/acquires the stable lock, reclassifies while holding it, publishes the exact init marker, creates the
empty object directory, and calls initialize_plain_control with the original deadline and cancellation flag.
The control owner closes and reads back its format metadata. Before the manifest, the root rechecks unfinished
state under root.lock and uses the bounded emptiness probe. Only then does it publish root.toml last.

## Owned metadata stages

The no-clobber helper uses tempfile 3.27.0 Builder with the full fixed basename as prefix, an empty suffix and
rand_bytes(0), then tempfile_in in the destination's parent. The inspected Windows and Unix backends use
create_new(true), so an existing stage is never truncated by the library. The helper bounds and reads back
staged bytes, syncs the file, and calls persist_noclobber. Existing final destinations require exact readback
and are never overwritten.

The fixed name alone is insufficient ownership. A stage must be a regular file containing the exact complete
metadata bytes or, only in the states below, an exact byte prefix of that finite metadata. This includes an
empty create-before-write stage. Unknown bytes, symlinks, extra files and arbitrary random/prefix-matching
temporary names are preserved and refuse initialization or discard.

| State | Recognized stages and explicit handling |
|---|---|
| Before exact init marker | Only root.lock and root.initializing.stage may exist. The stage is a byte prefix of the exact init marker. Explicit init takes the existing lock, reclassifies, removes that known stage and recreates it no-clobber. Discard refuses without the exact final marker. |
| Exact init marker, no manifest | The init stage, if present, must equal the complete marker. The manifest stage may be a byte prefix of the exact manifest. Retry or discard first proves empty stores under root.lock; only then can it remove these known metadata stages. |
| Exact final manifest and exact init marker | Both stages, if present, must contain their complete corresponding metadata. Inspection and ordinary open accept this Current layout without cleanup or an emptiness probe. Explicit init may remove these redundant stages under root.lock and still returns AlreadyExists. |

After successful publication the helper checks/removes the exact completed stage, including tempfile's
documented leftover hard link. A crash before cleanup leaves a recognized state above, rather than an unknown
root entry. A failed or uncertain publication is resolved by exact final readback; it never overwrites or
blindly replays. The init marker remains after final manifest publication. Directory sync is attempted on Unix;
Windows does not open a directory handle for it. These steps do not establish universal power-loss durability:
damaged bytes outside the exact metadata-prefix rules remain unsupported.

## Explicit discard

Discard requires an exact init marker, no final manifest, a recognized layout and no stored user data. It
checks object-directory emptiness without walking a corpus and accepts a control database only when absent,
zero length, or recognized as semantically empty by plain_control_is_empty while holding root.lock. ProbeLimit
and every other probe error abort without deletion. Unknown files, populated objects, unknown control tables,
corrupt control state and unsupported formats are preserved.

Discard validates every stage and the empty control/object state before deletion. It removes only the recognized
metadata stages, control file, empty store directories and init marker. It removes the marker last and never
removes root.lock. A lock-only or premarker-stage crash remnant may be retried by explicit initialization, but
does not authorize discard. No recursive deletion, store repair or import cleanup is performed.

## Archive and import boundary

Archive creation and import remain owned by stopped-root maintenance
([recipe](../search-retention/FUNCTIONS.md#backup-is-a-completed-archive-not-a-runnable-clone)). This root
owner only refuses the reserved archive/import marker names; it does not open, repair, resume or clear them.
The import owner must validate destination-bound marker bytes before importing.

## Open and shutdown

Open accepts only Current, acquires the existing lock without creating a file, and reclassifies under that lock
before returning the guard. Stores open afterward and must close before the guard drops. There is no
root-directory file handle, custom NT path, ancestor lock graph or silent legacy fallback.

## Guarded writable open

open_plain_with_recovery expresses repair permission, not an Index-specific constructor or routing wrapper.
Its current consumer is Index, including watch's owned Index children. It validates the existing layout,
takes root.lock and revalidates before any writable engine open. Ordinary read-only open/inspection/query
continue through their non-repairing paths and can return NeedsRepair.

On NeedsRepair, the root owner supplies an owned anonymous temporary file outside the root to
[recover_plain_control](../../search-control-redb/FUNCTIONS.md#guarded-engine-recovery). CONTROL validates
recovery/integrity and application format on that copy, rechecks the original, then recovers/clean-closes/
inspects the original through maintained redb. No candidate bytes are copied back, no root is reset and no
unknown/corrupt/unsupported format is initialized. The guard and original deadline/cancellation remain live.
This capability is not incomplete-initialization repair, source reconciliation or an alternate root lifecycle.

## Crate surface and physical removal

Only src/lib.rs and src/plain_root.rs remain. The public plain_root module and crate-root reexports expose
the operations above, including guarded writable opening, PlainRootStatus, PlainRootInspection, PlainRootGuard, PlainRootError,
PlainRootResult, and the existing layout constants LOCK_FILE, MANIFEST_FILE, INIT_MARKER,
INCOMPLETE_IMPORT_MARKER, OBJECT_DIR, CONTROL_DIR and CONTROL_DB_FILE.

The following legacy source files and their inline unit fixtures are physically deleted:

    src/admission.rs
    src/error.rs
    src/identity.rs
    src/lease.rs
    src/state.rs
    src/supervisor.rs
    src/transition.rs
    src/transition/acquire.rs
    src/transition/health.rs
    src/transition/release.rs

There was no separate crate tests directory. No legacy module or reexport is retained behind cfg(test).
The unused search-contracts, search-domain, search-ports and search-config dependency edges are removed;
the root crate depends directly only on search-control-redb and tempfile. Binary source removal is owned
separately and is not implemented inside this crate.

Root qualification and later S1/P1/Q1 exact-source scopes live in the existing
[registry](../../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status); this recipe is not a second acceptance ledger.

## Operator Qdrant storage

The application data root and the operator's Qdrant storage directory are different owners/locations.
Q1 keeps PlainRootGuard through publication/view output; Qdrant is started and stopped only by the operator,
not this root crate or the application.

Windows debug qualification observed storage-root depth sensitivity with the same server/driver/profile/
collection/corpus: a 76-character storage root failed Gridstore index creation with OS error 3; a 49-character
root succeeded. The final publisher run used a fresh 48-character operator storage root. These are measured
paths, not a universal MAX_PATH threshold, exact failing syscall/leaf diagnosis or application length rule.
No profile/index-memory/generation-name/global-registry workaround was introduced.

The failed original root remains intact with no committed current and an unresolved Create obligation.
Do not reset or adopt its target to hide the failure. Runtime cause/qualification details and remaining recovery
work belong to the registry and publisher; incomplete-root discard does not clear current user data.
