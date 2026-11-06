# Plain control metadata and current source rows

The retained CONTROL implementation owns control/control.redb through vendored redb 4.5.0. The existing plain_control_meta format marker and all root initialization, read-only inspection, empty-probe, savepoint, and incomplete-discard rules remain unchanged.

## Source operation

current_source adds three ordinary current tables on the first successful retained-source commit:

- source_state stores the source-row schema version and one persisted root-local SourceNamespaceId.
- current_source_rows stores typed CurrentSourceRecord values by independently generated SourceId.
- current_source_locators maps exact OS-native path bytes to a stored SourceId.

ReadOnlyDatabase::open and a read transaction capture the namespace, locator binding, row revision, and registration revision before file I/O. The native read-only open cannot repair or create the control database. No table is written during capture; an absent source schema only produces an in-memory capture for the later CAS. Unknown or partial table layouts are refused before source acquisition.

After the immutable object has been published and read back, commit_current_source opens one short write transaction in this same control.redb. It atomically compares the captured namespace and registration/source row revisions with the current locator mapping, then advances row_revision and writes the new ObjectRef and NativeFileKey. A stale or corrupt row is an error; the old object remains untouched and the new object may remain unreferenced.

Only commit_current_source opens Database for writing. An uncertain commit is observed through ReadOnlyDatabase::open and current row reads, without repair or writes.

Current source tables are ordinary Current-root user data. They do not change the initialization format marker. The existing unfinished-root emptiness probe still recognizes only its original empty states; any additional table or persistent savepoint remains nonempty and ineligible for discard. Regular source operations do not call the empty probe.

## Existing root-control operations

initialize_plain creates only the existing format metadata. inspect_plain_control reads the marker without enumerating the populated table catalogue. open_plain retains root.lock and verifies the current root before source work. No new root registration, key, credential, or lifecycle service is added.
