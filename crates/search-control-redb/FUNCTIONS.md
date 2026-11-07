# Plain control metadata, current sources and preparation

The retained CONTROL implementation owns control/control.redb through vendored redb 4.5.0. The existing plain_control_meta format marker and all root initialization, read-only inspection, empty-probe, savepoint, and incomplete-discard rules remain unchanged.

## Operation-scoped CONTROL writer

[ControlWriter](src/plain_control.rs) owns one existing validated redb Database for Index source/preparation,
staging and upload. The daemon transfers that same owner into publication instead of reopening/clean-closing
the database for every row/page. Borrowed methods create short read/write transactions; every write still uses
normal durable commit. No transaction spans source/object I/O, RPC, transform work or output. A live Database
between transactions is not a long-lived write transaction or another state/service layer.

The owner explicitly closes on success, error or cancellation before native read-only acceptance, final
publication CAS and PublishedView creation. Standalone native read-only captures remain separate entrypoints.
A failed borrowed commit returns Database: stop and close before observation. Reading the still-live database
cannot establish durable acceptance. Displaced unused path-opening publication APIs are removed, not wrapped.

## Source operation

current_source owns three ordinary current tables in the same database:

- source_state stores the source-row schema version, persisted root-local SourceNamespaceId and non-reset wanted_corpus_revision, plus durable admitted-selection records.
- current_source_rows stores typed CurrentSourceRecord values by independently generated SourceId.
- current_source_locators maps exact OS-native path bytes to a stored SourceId.

Standalone capture uses ReadOnlyDatabase::open and a read transaction; Index borrows a short read transaction from ControlWriter. Both capture namespace, locator binding, row revision and registration revision before file I/O. The native read-only open cannot repair or create the control database. No table is written during capture; an absent source schema only produces an in-memory capture for the later CAS. Unknown or partial table layouts are refused before source acquisition.

After the immutable object has been published and read back, commit_current_source opens one short write transaction in this same control.redb. It atomically compares the captured namespace and registration/source row revisions with the current locator mapping, then advances row_revision and writes the new ObjectRef and NativeFileKey for a changed head. Identical ObjectRef plus NativeFileKey returns the existing logical row without advancing its revisions/wanted guard; a key change remains an update even with the same bytes. A stale or corrupt row is an error; the old object remains untouched and the new object may remain unreferenced.

Standalone capture/observation uses ReadOnlyDatabase; the borrowed Index path uses ControlWriter transactions. commit_current_source owns the short source write. Standalone uncertain-commit observation occurs only after closing its database; a failed borrowed commit stops the owner before any later native read-only observation. Source-row decoding rejects ObjectKind::Prepared.

Current source tables are ordinary Current-root user data. They do not change the initialization format marker. The existing unfinished-root emptiness probe still recognizes only its original empty states; any additional table or persistent savepoint remains nonempty and ineligible for discard. Regular source operations do not call the empty probe.

capture_current_sources reads the current inventory natively read-only for explicit refresh.
remove_current_sources compares the captured full source rows/namespace/locator bindings, atomically removes
their current source/locator/preparation entries and advances the wanted-corpus guard. It does not delete
immutable objects, admission records or saved generation source/page/evidence rows. Only confirmed filesystem
absence in the refreshed scopes authorizes removal; excluded-but-present and inaccessible observations do not.

## Catalogue capacity and bounded reconciliation

Source schema v2 stores exact current_source_totals in source_state; legacy v1 remains readable only through
its supported bounded paths. The existing 4096-source / 16 MiB catalogue bounds are unchanged. Accounting
includes encoded source rows AND reverse-locator bytes, not Source object content size. Source mutations
compare guards and projected totals in the same transaction; inserts/replacements cannot commit an unreadable
catalogue. Current readers and publication paths enforce the same accounting even when publication's own
capture-byte charge is below its separate cap. Publication source_count is the captured Source inventory length,
including Empty/Gap inputs, not a point count. Mismatched v2 totals are corruption, not silently rebuilt counters.

Index completes bounded stable observations and preflight_current_sources before row mutations. Its projected
union includes outside-scope rows and only confirmed in-scope missing removals. The narrow bounded legacy
reconciliation route can remove those missing generated inputs and restore an over-cap catalogue; it does not
reset CONTROL, drop an unread subset or repair through query. Uncertain/unreadable observation authorizes no removal.
The Unix OsString import correction is source-only evidence; it is not a Unix build/native qualification.

## Durable selections

AdmittedSelection { path, kind: SelectionKind::File|Tree } records desired filesystem scope independently of
observed source rows. admit_selection persists bounded versioned native-locator/kind records in the existing
source_state table; identical admission is a logical no-op. Deletion does not clear admission or change its
known kind. No new table/database or root format is introduced.

capture_selections uses native read-only CONTROL. Legacy roots with no records return no admissions and are
not migrated by reading. Bare index turns that absence into INDEX_SELECTION_REQUIRED. A retain-file source row
is not an implicit admission. Admissions also do not prepare or hide unrelated current source rows from the
publication owner's full matching-preparation check.

## Current preparation

[current_preparation.rs](src/current_preparation.rs) adds the optional current_preparations table in this same database. Each SourceId row binds namespace/source identity, selected PreparationSettings with a non-reset settings revision, a non-reset outcome revision, and an optional bound Ready/Gap outcome. An outcome binds source-row revision, registration revision, exact input ObjectRef and settings; Ready references a Prepared object, while Gap contains one closed reason.

- ensure_preparation_settings initializes defaults only when no selection exists. It preserves an existing selection, its settings revision, stored outcome and outcome revision.
- Standalone capture_preparation uses a native ReadOnlyDatabase snapshot; ControlWriter::capture_preparation uses a short read snapshot on its owned database. Both capture source/settings/expected outcome together. It creates no database/table and performs no repair. Absent selection, missing source and malformed/unsupported schema remain errors.
- commit_preparation runs after object publication/readback and saved-artifact decoding. In one short transaction it compares the exact captured source row, namespace/registration/input, settings value/revision and expected outcome value/revision, including absence. A-B-A value equality cannot bypass non-reset revision checks.
- Identical outcomes preserve logical revisions. A late Gap cannot replace a usable Ready for the same wanted semantic binding. Any stale completion fails without changing the newer logical head; an already-written object may remain unreferenced.

Standalone preparation opens its database for settings/outcome writes; Index borrows the operation's ControlWriter. Failed borrowed commits stop and close before observation; standalone unknown-commit observation also follows close. No write transaction spans source/object/transform I/O. Read-only capture preserves physical bytes; writable stale rejection or a logical no-op can still change redb's physical file bytes.

Preparation rows are ordinary Current-root user data. They do not change the root format or weaken persistent-savepoint/incomplete-discard safety. [Daemon](../../bins/eliot-searchd/FUNCTIONS.md#prepare-source-path) is the actual caller; [unitizer](../search-prep/search-unitizer/FUNCTIONS.md) owns artifact encoding/validation, not CONTROL.

<a id="restartable-generation-input-capture"></a>
<a id="publication-state-and-snapshot"></a>
## Publication state and snapshot

[publication.rs](src/publication.rs) owns five ordinary tables in this same control.redb:

| Table | Actual responsibility |
|---|---|
| publication_state | Persisted random publication namespace, non-reset generation/current revisions, current pointer and one pending build. |
| generation_sources | Exact captured source/preparation rows, including explicit Gaps and completed source dimensions. |
| generation_pages | Fixed point/ObjectRef/unit/range descriptors, page boundaries and measured SDK request bytes; no unit text. |
| generation_evidence | Generation/point ID to source/input/prepared/unit/byte-line evidence. |
| retired_generations | Exact predecessor/abandoned target, ownership flag and any full unresolved pending-build header; cleanup obligation remains. |

ControlWriter::capture_corpus uses one short read snapshot on the owned database and bounded owned source metadata. Every retained source needs a matching preparation; no success-only scan. Source commits and actual settings/outcome changes advance wanted_corpus_revision in their existing transactions; identical settings/outcome no-ops do not. An old missing counter is baseline zero without capture writes. Publication's own staging writes do not advance this guard.

start_build compares captured namespace/wanted revision, current value/revision, pending absence and non-reset generation sequence. It reserves a never-reused sequence under the persisted random publication namespace. Captured source metadata and reference pages/evidence are staged in bounded transactions before remote mutations; complete_capture seals all sources. New ELSUPG02 pages encode 20 header bytes plus 176 bytes per point. Legacy ELSUPG01 pages remain read-only compatible: redundant text is framing-checked and discarded, never used as upload truth or rewritten during a read. Per-dispatch operation_revision rejects late callbacks for a newer attempt. Unknown dispatch remains unserved. supersede_unknown_build atomically checks the exact pending/current/source guards, stores its full unresolved build header in retired_generations, preserves all source/page/evidence rows and generation identity, then releases only the active pending slot for a never-reused successor. This is not settlement: cleanup refuses unresolved retired builds, including late old Create/Insert effects.

commit_generation compares the full expected pending record, current predecessor/revision and wanted guard; it switches current, records predecessor cleanup and clears pending in one short transaction. It does not copy generation rows. Object/backend I/O occurs outside write transactions. ControlWriter is closed before this final CAS; uncertain pointer commit is observed natively read-only after its database closes.

validate_generation and open_published invoke [the complete graph validator](src/publication/verify.rs) for publication/recovery acceptance in one native RO snapshot. It streams every saved source/page/evidence row, checks counts, exact IDs/ObjectRefs, source/Prepared bindings, ordinals and complete byte coverage, and rejects missing/orphan/malformed mappings independently of any user query. Only bounded per-source progress and one page are retained.

Ordinary search uses pin_published: bounded header/table checks open the committed pointer and native RO snapshot without a corpus scan. PublishedView::resolve_point point-reads each returned ID's evidence and CapturedSource from that same snapshot, checking generation/ordinal bounds and exact source/input/Prepared binding. It never looks up latest preparation. The query consumer then verifies the selected retained objects and all unit/range facts. An unrelated unreturned source body is not read; this does not certify the whole corpus afresh.

read_generation_source supplies the fixed decoder binding for publication page hydration, not a current-row substitution. The daemon keeps one verified Prepared buffer and borrowed decoded view across adjacent pages of the same source in each upload/readback pass, dropping it at the source boundary or pass end. ControlWriter closes before native read-only acceptance/final CAS/view creation; no write transaction spans object/backend I/O.

These tables remain ordinary user data: the root format, persistent-savepoint protection and unfinished-root empty probe are unchanged. The [daemon publisher](../../bins/eliot-searchd/src/published_corpus.rs) owns actual backend readiness/canary, snapshot installation and output. The retained [publication recipe](../search-index-qdrant/search-publication/FUNCTIONS.md) owns later convergence/view-lifetime rules.

## Raw Exact inventory

PublishedView::source_inventory reads the complete committed generation_sources inventory from the same pinned
native RO transaction, including Empty and PreparationGap sources without points. It validates captured source/
preparation bindings and generation counts; current unpublished source rows never replace this inventory.
The raw consumer selects from these captured Source ObjectRefs, not generation_evidence hits or latest current
preparation. An unknown requested SourceId fails; it does not widen selection. No source/preparation/publication
write, live-file fallback or Qdrant request is performed by this read-only Exact path.

## Plain CONTROL archive copy

copy_plain_control copies the supported closed CONTROL state under the caller's existing exclusive guard;
describe_plain_control independently verifies its bytes/format description. Archive export checks the copy,
typed reference closure and saved objects before completion publication. Copied pending/cleanup records are
evidence, not authority to mutate the original backend. This introduces no import/restore entrypoint or new table.

## Guarded engine recovery

Native read-only inspection reports NeedsRepair without repairing. The actual writable-open capability caller
holds a validated current root's exclusive guard and supplies an owned empty temporary file outside that root.
recover_plain_control reads the dirty original into this owned copy, uses maintained redb recovery/integrity
checks and validates application format there before original writes. It rechecks the original, recovers it
through maintained redb, clean-closes and inspects it. It never copies recovered candidate bytes back.

Unsupported/corrupt/missing format metadata is refused without reset; no format row, user table or persistent
savepoint is created/removed to bypass refusal. Original deadline/cancellation reach the engine repair callback.
Cancellation is not a rollback or byte-preservation promise once recovery of a valid original has begun.
Read-only source/preparation/query captures and incomplete-root emptiness/discard rules remain unchanged.

## Existing root-control operations

initialize_plain creates only the existing format metadata. inspect_plain_control reads the marker without enumerating the populated table catalogue. open_plain retains root.lock and verifies the current root before source work. No new root registration, key, credential, or lifecycle service is added.
