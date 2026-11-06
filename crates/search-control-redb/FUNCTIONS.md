# Plain control metadata, current sources and preparation

The retained CONTROL implementation owns control/control.redb through vendored redb 4.5.0. The existing plain_control_meta format marker and all root initialization, read-only inspection, empty-probe, savepoint, and incomplete-discard rules remain unchanged.

## Source operation

current_source adds three ordinary current tables on the first successful retained-source commit:

- source_state stores the source-row schema version and one persisted root-local SourceNamespaceId.
- current_source_rows stores typed CurrentSourceRecord values by independently generated SourceId.
- current_source_locators maps exact OS-native path bytes to a stored SourceId.

ReadOnlyDatabase::open and a read transaction capture the namespace, locator binding, row revision, and registration revision before file I/O. The native read-only open cannot repair or create the control database. No table is written during capture; an absent source schema only produces an in-memory capture for the later CAS. Unknown or partial table layouts are refused before source acquisition.

After the immutable object has been published and read back, commit_current_source opens one short write transaction in this same control.redb. It atomically compares the captured namespace and registration/source row revisions with the current locator mapping, then advances row_revision and writes the new ObjectRef and NativeFileKey. A stale or corrupt row is an error; the old object remains untouched and the new object may remain unreferenced.

Source capture/observation uses ReadOnlyDatabase; commit_current_source owns the source write. An uncertain commit is observed through native read-only current-row reads, without repair or writes. Source-row decoding rejects ObjectKind::Prepared.

Current source tables are ordinary Current-root user data. They do not change the initialization format marker. The existing unfinished-root emptiness probe still recognizes only its original empty states; any additional table or persistent savepoint remains nonempty and ineligible for discard. Regular source operations do not call the empty probe.

## Current preparation

[current_preparation.rs](src/current_preparation.rs) adds the optional current_preparations table in this same database. Each SourceId row binds namespace/source identity, selected PreparationSettings with a non-reset settings revision, a non-reset outcome revision, and an optional bound Ready/Gap outcome. An outcome binds source-row revision, registration revision, exact input ObjectRef and settings; Ready references a Prepared object, while Gap contains one closed reason.

- select_preparation_settings changes the settings revision only when the selected value changes. ensure_preparation_settings initializes defaults only if absent and preserves an existing selection. Neither clears the stored outcome or resets its revision.
- capture_preparation reads source/settings/expected outcome from one native ReadOnlyDatabase snapshot. It creates no database/table and performs no repair. Absent selection, missing source and malformed/unsupported schema remain errors.
- commit_preparation runs after object publication/readback and saved-artifact decoding. In one short transaction it compares the exact captured source row, namespace/registration/input, settings value/revision and expected outcome value/revision, including absence. A-B-A value equality cannot bypass non-reset revision checks.
- Identical outcomes preserve logical revisions. A late Gap cannot replace a usable Ready for the same wanted semantic binding. Any stale completion fails without changing the newer logical head; an already-written object may remain unreferenced.

Only explicit settings selection/initialization and outcome commit open a writable database for preparation. Unknown commit results are resolved by native read-only observation. No writer spans source/object/transform I/O. Read-only capture preserves physical bytes; writable stale rejection or a logical no-op can still change redb's physical file bytes.

Preparation rows are ordinary Current-root user data. They do not change the root format or weaken persistent-savepoint/incomplete-discard safety. [Daemon](../../bins/eliot-searchd/FUNCTIONS.md#prepare-source-path) is the actual caller; [unitizer](../search-prep/search-unitizer/FUNCTIONS.md) owns artifact encoding/validation, not CONTROL.

<a id="restartable-generation-input-capture"></a>
## Future Q1 generation capture

P1 current preparations are not a published generation or complete inventory capture. Q1 must connect bounded fixed generation input, staging evidence and atomic matching current/evidence state before installing a view, as required by the retained [publication recipe](../search-index-qdrant/search-publication/FUNCTIONS.md#fixed-pending-generation-input). Those generation tables and publisher are not implemented by this P1 cut. Do not infer publication from a Ready preparation or introduce a second ledger.

## Existing root-control operations

initialize_plain creates only the existing format metadata. inspect_plain_control reads the marker without enumerating the populated table catalogue. open_plain retains root.lock and verifies the current root before source work. No new root registration, key, credential, or lifecycle service is added.
