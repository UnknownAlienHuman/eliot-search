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
The committed manifest bytes are
format = "eliot-plain-root"\nversion = 1\n. The owned init marker is
eliot-plain-root-initializing-v1\n.

| Observation | Status |
|---|---|
| Missing or empty selected directory | Absent or Empty |
| Only a regular root.lock, or exact marker plus recognized empty stores and no manifest | IncompleteInitialization |
| Exact manifest, required layout, stable lock file and a control database accepted by the control owner's readonly validator | Current |
| Missing committed files, damaged recognized metadata, or nonempty stores before manifest publication | Corrupt |
| Unknown entries, invalid marker, symlinks or unsupported/legacy/encrypted format | Unsupported |
| Another process holds the root or control lock | Busy |
| Reserved archive.complete entry | Archive |
| Reserved root.importing entry | IncompleteImport |

Unknown or unsupported roots are classified before initialization creates a lock or other file. Existing roots
are never repaired by inspect or open. Current validates the manifest and calls inspect_plain_control; it does
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
and then calls plain_control_is_empty. That bounded probe uses the same opened file for its 64 MiB byte ceiling
and metadata check. Extra user tables or metadata report nonempty, while hitting the ceiling returns the
distinct ProbeLimit error. Zero-length staging is accepted as empty. This module never parses or copies
database bytes. The legacy redb 2.6.3 dependency remains with legacy consumers only.

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

The shared no-clobber helper uses tempfile 3.27.0 in the destination's parent, bounds and reads back staged
bytes, syncs the file, and calls persist_noclobber. Existing destinations are accepted only after exact
readback. The init marker remains after manifest publication so the manifest stays the final published file;
a valid manifest plus the exact marker classifies as Current. Directory sync is attempted on Unix. Windows
does not open a directory handle for this step. These operations do not establish universal power-loss
durability or atomic cleanup on every filesystem.

## Explicit discard

Discard requires an exact init marker, no final manifest, a recognized layout and no stored user data. It
checks object-directory emptiness without walking a corpus and accepts a control database only when absent,
zero length, or recognized as semantically empty by plain_control_is_empty while holding root.lock. ProbeLimit
and every other probe error abort without deletion. Unknown files, populated objects, unknown control tables,
corrupt control state and unsupported formats are preserved.

Discard removes only the recognized control file, empty store directories and init marker. It removes the
marker last and never removes root.lock. A lock-only crash remnant may be retried by explicit initialization,
but lock-only state does not authorize discard. No recursive deletion, automatic recovery or import cleanup is
performed.

## Archive and import boundary

Archive creation and import remain owned by stopped-root maintenance
([recipe](../search-retention/FUNCTIONS.md#backup-is-a-completed-archive-not-a-runnable-clone)). This root
owner only refuses the reserved archive/import marker names; it does not open, repair, resume or clear them.
The import owner must validate destination-bound marker bytes before importing.

## Open and shutdown

Open accepts only Current, acquires the existing lock without creating a file, and reclassifies under that lock
before returning the guard. Stores open afterward and must close before the guard drops. There is no
root-directory file handle, custom NT path, ancestor lock graph or silent legacy fallback.

The implementation has not been built or run through native Windows scenarios in this slice. Those checks
remain with the verifier after the root and control changes are both ready.
