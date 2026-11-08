# Daemon — retained input, publication and read-only search

The daemon owns native invocation, bootstrap settings, root dispatch and factual output. Retention, preparation and publish-prepared use the existing root-owned CLI child; --search-stdio services one framed query with one joinable synchronous worker.

## Index and refresh path

    eliot-searchd --index [--index-path PATH] --qdrant-url URL [--data-root ROOT] [--config-file PATH]

[native_root_command.rs](src/native_root_command.rs) calls open_plain_with_recovery once for Index;
initialization remains explicit. This is guarded repair-permitted opening, not command routing inside the root
owner. Ordinary open/query remain read-only. index_sources carries that same guard and original 120-second deadline through scan, typed retain/
prepare, membership update, publication and output. It does not chain JSON-producing commands or reacquire
the root between stages. After initial inventory/selection/scan, Index opens one ControlWriter and borrows
short transactions for source/preparation work, then transfers it into publication staging/upload. The Database
may remain open across object/RPC work; transactions may not. Failed borrowed commit stops the operation;
explicit close precedes native read-only acceptance, final CAS and PublishedView.

The pinned ignore 0.4.34 walk uses captured CWD, strict ignore-file I/O, standard filters, no link following,
and explicit .git/data-root exclusion. Reparse/link and incomplete/malformed-ignore observations refuse the
refresh; a second matching scan precedes removals. Only confirmed in-scope filesystem absence removes current
membership; an ignored but present file is not removed. Outside source rows and retained/published evidence remain.
An unchanged NativeFileKey/ObjectRef preserves the logical source head; a key change is a real update even
when content is identical. Typed preparation preserves matching outcomes/settings before publication.

index PATH persists an AdmittedSelection and refreshes only that scope. Bare index calls capture_selections
at writable startup, unions every admitted File/Tree and deduplicates native locators. Explicit File admission
survives a containing Tree's ignore rules. Both scan passes own bounded union maps and small root/kind/missing
metadata per selection, not complete file-list copies per scope. This bound is source-reviewed, not a native
performance qualification.

Admission/kind remain after absence/re-add. No admission returns INDEX_SELECTION_REQUIRED; legacy source rows
are not promoted, implicitly prepared or omitted from complete publication checks. Foreground CLI watch now
repeats saved-selection Index with no overlap and a new original budget per iteration. Its internal
--index-owner-stdio flag installs a process-lifetime stdin reader: EOF/read failure requests cooperative
cancellation, including when the parent disappears. This flag is Index-only and creates no public protocol.
Query and writer retain root exclusion through output; no concurrent view manager is required by this cut.

## Catalogue and exclusion boundaries

Index uses the existing v2 source totals, full projected-union preflight and bounded confirmed-missing legacy
reconciliation before row writes. Source/publication readers count locator bytes too. A v2 accounting mismatch
is corruption; no query-side reset/repair or silent omission occurs. Data-root and candidate exclusion use
resolved filesystem paths while stored native locator identity is unchanged. The Unix OsString import fix has no Unix run.

## Archive export path

    eliot-searchd --archive-export --archive-path OUT [--data-root ROOT] [--config-file PATH]

[archive_export.rs](src/archive_export.rs) takes stopped-root exclusion/open_plain_with_recovery and verifies
the rooted object closure before creating fresh OUT. It copies CONTROL and required final objects only, writes
archive.header (plain v1, served baseline), then verifies CONTROL/objects/bindings from the copy with the shared
read_object_in surface. Source/OUT equality/nesting and parent reparse are refused. No runnable root.toml,
root.lock, initialization marker, scratch or Qdrant storage is copied. Files/directories are flushed with the
root owner's primitive, archive.complete is written last and the result is verified again. The original root
and external backend are not reset/adopted. Interrupted exports do not become complete archives.

## Archive import path

    eliot-searchd --archive-import --archive-path ARCHIVE --data-root DEST --qdrant-url URL [--retry]

[archive_import.rs](src/archive_import.rs) validates archive closure and the whole served inventory/caps before
DEST effects. Fresh initialization or exactly bound open_plain_marked holds one stable guard. The current
111-byte marker binds canonical destination, archive CONTROL digest and served generation; marker/stage bounds
remain 256 bytes. Only selected served objects/rows are installed through ControlWriter::install_served_corpus.
Portable source/preparation facts and valid older bindings survive; native keys are historical and admissions
are absent. No secret reprepare or old publication authority is imported.

Normal publication builds a fresh destination namespace/generation, verifies complete current/evidence against
the selected baseline and closes its view/stores before clear_import_marker. Exact retry preserves installed
logical state; postcurrent marker-clear failure can reuse current, while uncertain Create can require a fresh
successor with the old unresolved target retained. Wrong archive/destination and populated markerless retry
refuse. The [maintenance owner](../../crates/search-runtime/search-retention/FUNCTIONS.md#explicit-restoreimport-into-a-fresh-destination)
owns the complete rules and registry owns the separate native/UNKNOWN scopes.

## retain-file path

1. Resolve --source-file against the captured startup directory while preserving native path bytes.
2. Open the existing root with open_plain and retain PlainRootGuard.
3. Capture a read-only current-source guard from the same root control.redb.
4. Call search-safe-reader acquire_file once for the selected path.
5. Supply the acquired buffer as a byte slice to search-revision-store, publish one versioned source ObjectRef beneath guard.object_dir, and read the object back to verify exact bytes, length, and BLAKE3.
6. Commit one CurrentSourceRecord with namespace, random SourceId, NativeFileKey, ObjectRef, and row/registration CAS in the same control.redb.
7. Emit a compact factual JSON result only after the row commit. It reports IDs, revisions, file key, ObjectRef fields, and verified readback; it does not report search evidence.

Retain-file has an 8 MiB bound and does not execute, transform or index the input. Failures are content-free and never become successful empty results.

## prepare-source path

    eliot-searchd --prepare-source --source-id UUID [--unit N] [ROOT] [--data-root ROOT] [--config-file PATH]

native_root_command dispatches to [prepared_source.rs](src/prepared_source.rs). With PlainRootGuard held:

1. Ensure settings exist without replacing an existing selection; capture source, settings and expected outcome from one native read-only CONTROL snapshot.
2. Reuse a Ready artifact matching that input/settings, or read the retained Source ObjectRef and call prepare_utf8. The live locator is never opened.
3. Publish the complete encoded artifact as ObjectKind::Prepared through store_object. Read the stored object and decode_prepared_utf8 before the short guarded outcome commit.
4. Emit only the decoded stored artifact's selected complete text/ranges after CAS succeeds. Empty UTF-8 has an actual zero-unit artifact; binary/invalid UTF-8 and selected ceiling refusals become distinct bound Gaps. Integrity, cancellation, deadline and stale errors remain errors.

Input/output text ceilings are 8 MiB; encoded artifact allowance is 64 MiB, with at most one million lines/units. Execution ceilings are separate from output-affecting PreparationSettings. N defaults to zero. An unavailable ordinal returns PREPARATION_UNIT_OUT_OF_RANGE; it does not create a partial unit. Ready reuse still verifies the addressed artifact; missing/corrupt objects do not trigger rebuilding or live-source fallback.

Preparation stdout is bounded at 397312 bytes for one full 64 KiB unit under worst-case JSON escaping plus metadata/framing. Other root/retention stdout and stderr bounds remain unchanged. Root guard, original deadline/cancellation and child cleanup cover the operation through output.

No operation executes input or implicitly starts a network/model/Qdrant service. Root initialization, inspection, persistent-savepoint safety and incomplete discard are unchanged. [CONTROL](../../crates/search-control-redb/FUNCTIONS.md) owns CAS; [unitization](../../crates/search-prep/search-unitizer/FUNCTIONS.md) owns the complete artifact.

## publish-prepared path

    eliot-searchd --publish-prepared --qdrant-url URL [ROOT] [--data-root ROOT] [--config-file PATH]

[published_corpus.rs](src/published_corpus.rs) holds PlainRootGuard and the original 120-second deadline/cancellation:

1. Capture every current source and matching preparation, including empty/Gap; refuse absent/stale preparation.
2. Persist captured source metadata, compact reference pages and matching evidence in bounded CONTROL writes.
   Hydrate actual upload/readback text from immutable Prepared objects with the captured decoder binding.
   Each pass retains one verified buffer/decoded view across adjacent pages of that source and releases it on
   source change or pass end. Check all unit/range facts and the exact SDK encoded request bytes.
3. Build/verify the vectorless generation through the bridge: InsertOnly/wait, full schema/point/payload/count
   readback and independent source-backed singleton TextQuery canary.
4. Compare/commit current, predecessor cleanup and pending clear atomically; open the completely validated
   matching native RO snapshot before reporting generation state/counts. The shared ControlWriter closes before
   acceptance/CAS/view; no write transaction spans object/backend I/O.

Publication accepts no --query/--limit and returns no user hits. Canary candidates are bounded complete
Unicode alphanumeric/underscore identifiers containing an alphanumeric character, including code/compact JSON.
Exact expected ID/source and finite positive score are required. Empty responses never prove tokenlessness.
Only source-backed absence of alphanumeric characters throughout the actual corpus permits the tokenless case;
no bounded known candidate in otherwise searchable text refuses readiness. True zero-point generations require
complete inventory/evidence/schema and exact count zero, without a sentinel.

The operator owns loopback Qdrant/storage. Existing ELIOT_QDRANT_API_KEY is consumed without logging.
Unknown mutations retain pending targets unserved. See [CONTROL](../../crates/search-control-redb/FUNCTIONS.md#publication-state-and-snapshot)
and [operator storage guidance](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md#operator-qdrant-storage).

## Read-only search path

    eliot-searchd --search-stdio [--qdrant-url URL] [ROOT] [--data-root ROOT] [--config-file PATH]

[framed_search.rs](src/framed_search.rs) services Query/Cancel while one joinable synchronous worker runs
[verified_query.rs](src/verified_query.rs) outside the I/O runtime. The worker opens the root and pin_published,
which does bounded committed-header/table checks without a complete generation scan. It queries the exact
committed collection, preserves backend order and point-resolves only returned evidence plus CapturedSource
from that same native RO snapshot.

Group returned points by captured binding. Verify both Source and Prepared ObjectRefs by actual length/BLAKE3,
decode with the captured source/settings and compare source/prepared bytes plus every selected unit's ordinal,
byte/line range and boundary facts. Emit the exact full UTF-8 unit and citation. A missing returned Source or
Prepared object is an explicit item Gap; corruption/routing/range mismatch is a typed failure. Unreturned source
bodies are not read. A newer current preparation does not replace the pinned capture.

Search never writes CONTROL/collections, publishes, prepares, reconciles or opens live source paths.
Root/view stay held through complete terminal flush; the worker is joined before teardown.
The [protocol](../../crates/search-provider-protocol/FUNCTIONS.md) admits one correlated terminal and caps total
framing at 8 MiB with whole-card OutputLimit. Do not add detached timeout workers or a session framework.

## Read-only query summary

[framed_search::query_summary](src/framed_search.rs) handles include_summary for Search/Raw/DecodedUtf8 and
session Open. Published counts/revision come from the pinned generation; selected counts/gaps use the original
selected published inventory. PublishedView::current_wanted_revision observes current CONTROL in the same RO
transaction with O(1) metadata work and no SourceRow/FS scan. It does not replace evidence or certify live files.

Metadata-only Database/UnsupportedSchema/corruption maps to the protocol's closed Unavailable reason while
valid retained results survive. Original cancellation/deadline and evidence/object integrity failures still fail.
The window caches this header unchanged; codec fitting charges actual summary bytes under the existing frame cap.
Source9cfa candidate qualification is recorded in the [registry](../../docs/audit/ANALYSIS_REGISTRY.md#a10-v1-candidate),
not promoted into a main/whole-product claim.

## Saved captured-source hints

include_source_info is a default-false private request/Open flag. [exact_literal::source_info](src/exact_literal.rs)
decodes the saved native locator from the published CapturedSource. Ranked queries use the source resolved with
the returned point; Raw/Decoded use selected committed source entries. Neither path substitutes current source
rows or performs live stat/open/canonicalization for the hint.

Available saved_path_hint always carries live_status: not_checked. Invalid locator/platform, non-Unicode path,
allocation or reply-budget failure is typed Unavailable; it does not suppress retained-object/range failures.
False/omitted requests retain legacy output without annotations. Optional hint bytes count within existing private
frame limits; the hint never becomes evidence identity or a separate public source_info RPC.

The new [MCP binary](../eliot-search-mcp/FUNCTIONS.md) is a separate rmcp JSON-RPC consumer over QueryClient /
OwnedQuerySession. eliot-searchd remains the private owned-child endpoint, not an MCP server by renaming its argv.
Candidate3 has CORE/MCP SOURCE PASS with all reviewed P2 items closed. Native five-client delivery remains
IN_PROGRESS in [AD1](../../docs/audit/ANALYSIS_REGISTRY.md#agent-delivery); no public client runtime acceptance is claimed.

## FIRST RAW execution

The same serviced framed worker accepts RawExactRequest and calls [raw_exact.rs](src/raw_exact.rs) instead
of the backend-query branch. Qdrant is never connected for this operation. The read-only root/view stay held
through the terminal, and the worker is joined. It obtains the entire committed source inventory through
PublishedView::source_inventory, including Empty/PreparationGap inputs, then applies the requested source subset.
Latest current rows, live paths and Prepared/Qdrant point inventory do not replace those raw Source bindings.

One private literal matcher is compiled per request, preserves overlaps/block state and resets between sources.
One retained object at a time is length/BLAKE3 verified before any occurrence becomes evidence. Output contains
RawSourceRef/revisions/ObjectRef/start/end/bytes_hex. Missing Source is a Gap; corruption is a failed terminal.
The <=64 item prefix does not stop scanning; scan_complete/complete_negative use actual whole-inventory work.
The literal is a nonempty UTF-8 String <=4096 bytes, not arbitrary non-UTF-8 needle input; framing remains 8 MiB.
Raw Cancel and short remaining-duration cases use the production wire; they do not qualify future session
held-output/window behavior. The separate retained DecodedUtf8 operation is described below; extracted and
Continue/Expand/expiry remain unimplemented.

## Retained DecodedUtf8 execution

The same worker dispatches DecodedUtf8Request to [decoded_exact.rs](src/decoded_exact.rs), without Qdrant.
It uses the pinned committed source/preparation/settings inventory. PreparationGap produces PREPARATION_GAP
with no PreparedRef. For Ready, it reads/verifies both Source and saved Prepared ObjectRefs, decodes the actual
artifact and compares its full UTF-8 bytes with Source. Only that equality authorizes identity source ranges.
No source decoding/materialization, normalization, reprepare, current/live fallback or artifact regeneration occurs.

The matcher scans the entire borrowed saved representation across unit boundaries. Matches return both refs,
source/representation byte ranges and exact text, preserving BOM/CRLF. Missing Ready Source/Prepared objects
are explicit gaps retaining the selected PreparedRef; integrity failure is fatal. Missing Prepared does not
prevent the separate Raw operation from reading verified Source bytes.
Existing literal/item/frame bounds and original deadline/cancellation apply. Output prefix limits do not end
representation scanning; any representation gap prevents complete negative. Session/window held-output,
Continue/Expand/expiry and extracted producer/route remain outside this implemented slice.

## Verification state

See the [Source recipe](../../crates/search-source/README.md) for the retained-byte/current-row contract and the [registry](../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status) for exact binary/library revisions, executed native cases, NOT_RUN boundaries and final gate results. Read-only refusal byte preservation must not be extended to a stale writable CAS transaction.
