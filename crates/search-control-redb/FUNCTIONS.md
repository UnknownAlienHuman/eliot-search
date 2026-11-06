# Plain control metadata, current sources and preparation

The retained CONTROL implementation owns control/control.redb through vendored redb 4.5.0. The existing plain_control_meta format marker and all root initialization, read-only inspection, empty-probe, savepoint, and incomplete-discard rules remain unchanged.

## Source operation

current_source adds three ordinary current tables on the first successful retained-source commit:

- source_state stores the source-row schema version, persisted root-local SourceNamespaceId and non-reset wanted_corpus_revision.
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
<a id="publication-state-and-snapshot"></a>
## Publication state and snapshot

[publication.rs](src/publication.rs) owns five ordinary tables in this same control.redb:

| Table | Actual responsibility |
|---|---|
| publication_state | Persisted random publication namespace, non-reset generation/current revisions, current pointer and one pending build. |
| generation_sources | Exact captured source/preparation rows, including explicit Gaps and completed source dimensions. |
| generation_pages | Complete immutable upload pages with actual unit text and range bindings. |
| generation_evidence | Generation/point ID to source/input/prepared/unit/byte-line evidence. |
| retired_generations | Exact predecessor or abandoned target with proven remote-ownership flag and cleanup obligation. |

capture_corpus uses one native RO snapshot and bounded owned source metadata. Every retained source needs a matching preparation; no success-only scan. Source commits and actual settings/outcome changes advance wanted_corpus_revision in their existing transactions; identical settings/outcome no-ops do not. An old missing counter is baseline zero without capture writes. Publication's own staging writes do not advance this guard.

start_build compares captured namespace/wanted revision, current value/revision, pending absence and non-reset generation sequence. It reserves a never-reused sequence under the persisted random publication namespace. Source inputs and full pages/evidence are staged in bounded transactions before remote mutations; complete_capture seals all sources. Per-dispatch operation_revision rejects late callbacks for a newer attempt. Unknown dispatch remains pending and unserved.

commit_generation compares the full expected pending record, current predecessor/revision and wanted guard; it switches current, records predecessor cleanup and clears pending in one short transaction. It does not copy generation rows. Object/backend I/O occurs outside writers; uncertain pointer commit is observed read-only.

validate_generation and open_published invoke [the complete graph validator](src/publication/verify.rs) in one native RO snapshot. It streams all saved source/page/evidence rows, checks counts, exact point IDs, ObjectRefs, source/Prepared bindings, ordinals and complete byte coverage, and rejects missing/orphan/malformed mappings independently of user hits. Only bounded per-source progress and one page are retained. PublishedView owns this snapshot; evidence resolves from its generation, not latest current preparation. All writer handles close before the view opens.

These tables remain ordinary user data: the root format, persistent-savepoint protection and unfinished-root empty probe are unchanged. The [daemon publisher](../../bins/eliot-searchd/src/published_corpus.rs) owns actual backend readiness/canary, snapshot installation and output. The retained [publication recipe](../search-index-qdrant/search-publication/FUNCTIONS.md) owns later convergence/view-lifetime rules.

## Existing root-control operations

initialize_plain creates only the existing format metadata. inspect_plain_control reads the marker without enumerating the populated table catalogue. open_plain retains root.lock and verifies the current root before source work. No new root registration, key, credential, or lifecycle service is added.
