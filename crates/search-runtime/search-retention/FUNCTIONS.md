# Stopped-root maintenance and plain archives

This recipe owns local-object reachability, archive and import. [Publication](../../search-index-qdrant/search-publication/FUNCTIONS.md)
owns live Qdrant retirement. [Local scope](../../../docs/architecture/LOCAL_SCOPE.md) selects plain storage, not key migration.
The unused coordinator Rust package, manifest and exclusive fixtures remain removed. The stopped-root
collect-objects, archive-export and archive-import slices are delivered only in their recorded Windows/Release
source/native scopes. M1 is DONE in that selected scope; V1/installed final-head and whole-product qualification
remain unfinished.

## No maintenance scan in the query path

Query reads only selected evidence and reports actual owner state. Do not recursively audit objects or replay logs
just to print a status/page. Full inventory is an explicit diagnostic/maintenance action, not a runtime credential.
Disk pressure pauses writes and reports maintenance; it does not authorize online local-object deletion.

## Stopped-root local-object collection

Stop watch/daemon readers and writers before this explicit maintenance operation:

    eliot-search collect-objects [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]
    eliot-searchd --collect-objects [--data-root ROOT] [--config-file PATH]

The CLI uses its owned child and closed collected summary. [collect_objects.rs](../../../bins/eliot-searchd/src/collect_objects.rs)
opens open_plain_with_recovery and retains the existing exclusive root guard through output. Busy refuses,
not kills another process. One supported consistent CONTROL snapshot seeds typed disk-backed ObjectMarks in
disposable redb scratch outside final objects; no new CONTROL table, format or durable collection ledger is added.
The existing original deadline/cancellation applies throughout.

### One complete mark

Mark current Source heads plus stale/Gap preparation inputs and Ready artifacts, all remaining published/
pending/partial-retirement/unresolved source/page/evidence references. Inspect every rooted Prepared artifact
with the real codec/binding checks and include its embedded Source ObjectRef before verifying all Source leaves.
Do not substitute a newer current head for an older artifact's input. No process-local window remains under
stopped-root exclusion, and marks are not reconstructed from history or supplied manifests.
Ordinary opening/collection refuses root.importing and its fixed stage. Only the explicit archive-import
consumer may resume an exactly bound incomplete destination; it never collects a partial import.

Validate all typed tables/references; missing/corrupt/unknown/unreadable or over-limit input stops before deletion.
Mark exact ObjectRefs once in disposable bounded scratch data outside final objects. Raw bodies are leaves.
An Immediate checkpoint after every 32 successful scratch commits releases pending redb bookkeeping;
this cadence never limits/omits marked objects and is not an object quota. The fixed f018 bound is source-reviewed,
not a universal native RAM-peak benchmark. Normal object/storage/encoder/preparation/query formats are unchanged.
Complete the entire metadata/Prepared closure and verify every marked object before sweep. Missing/corrupt
rooted references stop before deleting any final object; a partial mark authorizes no safe-subset deletion.

### One sweep

Perform one streaming enumeration of final objects. Delete only unmarked canonical known-version regular
non-reparse leaves after identity, length and BLAKE3 ownership checks. Preserve foreign/future-version files,
directories, wrong-hash candidates and uncertain identities; an unmarked integrity mismatch does not establish
ownership. Rooted corruption above is a failure, not a preserved-unmarked shortcut.
Close/remove only owned scratch on ordinary success/error. Before-sweep interruption deletes no final object;
a sweep-time filesystem failure may leave garbage and the next run rebuilds marks. No sweep journal or erasure proof.

This is cooperative stopped-root exclusion in account-private storage. The final unlink is pathname-based;
the guard/checks do not make check/unlink atomic against a noncooperating same-account external rename/reparse.

## Backup is a completed archive, not a runnable clone

Hold the source's exclusive root guard and closed normal stores. Create a separate archive outside the source root;
reject equal/nested source/archive/destination paths which would cause recursive copying or overwrite.
From the first byte use a non-runnable archive layout: no live root manifest/signature in a normal-open location.
A partial or marker-stripped archive must remain unopenable as an installation.

Copy supported committed control data and every required final object: current source/preparation, archived served
inventory/evidence and unresolved pending/cleanup references as non-executable evidence. Exclude locks, sockets,
scratch, transient stages and Qdrant collections. Archive fields never become arbitrary filesystem write paths.

Verify the copy's control snapshot and every referenced object from the copy itself. Flush supported data/directory
state; publish the small archive-complete marker last. No marker means incomplete, not an ordinary data root.
Record the actual durability limit. Normal root open refuses before recovery/backend work; the archive reader
never calls ordinary root open. Preserve original/archive. A raw OS clone is unsupported, not automatically adopted.

## Implemented archive-export consumer

    eliot-search archive-export --archive-path OUT --data-root ROOT [--daemon PATH]

[archive_export.rs](../../../bins/eliot-searchd/src/archive_export.rs) uses stopped-root exclusion and the actual
typed closure. The selected export baseline is served, recorded with format/version, served generation,
CONTROL length/digest and closure count in archive.header. A fresh non-overlapping OUT contains control/ and
objects/, not runnable root metadata/lock or backend data. Required current/stale/published/pending/cleanup
objects are verified; unrelated final orphans/foreign entries are not made archive authority.

The exported copy's CONTROL and required objects/bindings are verified from the copy itself, using shared
read_object_in without ordinary root adoption. After supported file/directory flushes, archive.complete is
published last and verified. Incomplete/missing/forged completion and normal-open refusal stay distinct from
import support. The directory flush primitive does not claim universal power-loss durability.

Archive 06f A-D, 7a7d v2 positive/Busy/incomplete-scan and final 597 interruption retain separate native scopes
in the registry. The earlier interrupted-test setup lacked root.lock before OUT and is not interruption proof.
A killed exporter may leave owned scratch outside the source/OUT; the reported forensic scratch is neither a
final object nor archive payload. This is not a resume journal or permission to reset original state.

## Explicit restore/import into a fresh destination

    eliot-search archive-import --archive-path ARCHIVE --data-root DEST --qdrant-url URL [--retry] [--daemon PATH]

[archive_import.rs](../../../bins/eliot-searchd/src/archive_import.rs) is the actual owned-child consumer:

1. Verify the complete archive closure, then validate its whole SERVED inventory under both Source catalogue
   and publication metadata caps before DEST effects. Corrupt unserved references also refuse. The selected
   baseline is served only; current desired rows are not mixed in and no secret preparation/live read occurs.
2. Initialize a fresh non-overlapping destination under the stable root guard. Publish root.importing before
   rows/objects/backend effects. Its current 111-byte binding covers the lossless canonical destination digest,
   archive CONTROL digest and served generation. Marker/stage bytes are bounded at 256; this is no RootId.
3. Copy only the selected baseline's retained closure. install_served_corpus preserves portable Source namespace/
   IDs/revisions, settings and outcomes, including an older binding when its input/settings match and bound
   revision is not newer than captured Source. Future bindings refuse. Native keys are historical; admissions
   are not copied. Source schema v2 totals are installed and local wanted advances once, not to archived wanted.
4. Publish using a fresh destination PublicationNamespace and never-reused backend names. Copied archive pending/
   cleanup rows authorize no original backend effects. Destination uncertainty uses the normal publication owner;
   an unknown old target can remain unresolved while a fresh successor is accepted.
5. Verify destination current/evidence, complete served inventory and retained bindings. Close stores/views before
   clearing the exact marker with the same guard; flush/readback and normal-root validation precede current use.

--retry uses open_plain_marked and shared guarded recovery. An identical installed baseline preserves Source/
preparation/totals/wanted and existing destination publication obligations. After current commit but failed marker
removal, retry may reuse that generation; physical CONTROL bytes need not stay identical. Wrong archive or copied
destination bindings refuse. A markerless destination is accepted only with proven empty initialized stores and
an absent/exact-prefix fixed stage; populated markerless roots are never adopted. Explicit retry does not discard
data, reset unresolved effects or create a restore journal/platform.

<a id="protected-storage"></a>
## Old protected data

Application encryption is not selected. Reject unsupported encrypted archives/roots before writes and leave bytes
and keys intact. Do not silently downgrade, generate replacement keys, reset counters, rewrap or migrate them.
An explicit real user-data export request is separate work, not a prerequisite for the plain product.

## Implemented checks and remaining M1

Sol SOURCE PASS at f01809bc follows the rejected old dbf bookkeeping bound; current control/store/unitizer/
both-bin locked checks and strict Clippy plus RELEASE builds passed. Windows/Release native A used an independent
read-only oracle for 15 rooted refs, including stale C/D revision-1 inputs behind current revision 2. One valid
store-created Source orphan was deleted (deleted=1, preserved=19); rooted data, foreign entries and CONTROL
were retained and Raw reopened. Missing C1 Source (27 bytes) returned OBJECT_MISSING before further deletes;
corrupt D1 Prepared (296 bytes) returned OBJECT_INTEGRITY_FAILURE before changes, preserving the orphan.
Observed waits/both EOF/OS absence belong to the recorded native scopes. Additional guard/refusal checks are
recorded in the [registry](../../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status); older dbf runs stay older evidence.

Import source 708621c1 has Sol SOURCE PASS, changed-owner locked check/strict Clippy/RELEASE build success and
selected native served/empty/corrupt, older/future binding, interrupted-copy, marker-clear retry and uncertain-backend
proof. Earlier d5 cases retain their own provenance. Exact outcomes/UNKNOWN observers are in the
[import checkpoint](../../../docs/audit/ANALYSIS_REGISTRY.md#m1-import-delivery).

M1 GC/export/import is DONE only in its selected Windows/Release scopes. V1 is READY: bounded known-freshness/
preparation-gap summary, real-repository relevance, ordinary-repo-1 scale and isolated installed/final-head
qualification are next. Whole-product/installed PASS and cross-OS native qualification are not established.
