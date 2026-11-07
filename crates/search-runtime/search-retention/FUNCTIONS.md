# Stopped-root maintenance and plain archives

This recipe owns local-object reachability, archive and import. [Publication](../../search-index-qdrant/search-publication/FUNCTIONS.md)
owns live Qdrant retirement. [Local scope](../../../docs/architecture/LOCAL_SCOPE.md) selects plain storage, not key migration.
The unused coordinator Rust package, manifest and exclusive fixtures remain removed. The stopped-root
collect-objects and archive-export slices are connected at their recorded source/native scopes. Import remains
unimplemented; M1 is PARTIAL and V1/installed final-head qualification is unfinished.

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
root.importing is currently reserved/unsupported: ordinary opening refuses it before collection. This is not
an implemented import flow; future supported import marking remains subject to the complete-reference rule.

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

1. Read the complete archive without writes or original backend effects. Validate format, references, exact objects,
   representation/gaps and bounded paths. Missing/unsupported/corrupt input fails, not rereads of changed live files.
2. Under the destination's exclusive root guard, initialize an empty plain root. Persist a destination-bound
   incomplete-import marker before importing rows/objects or issuing RPCs. Normal open refuses it before recovery.
3. Use the destination object owner for exact bytes, preserving portable Source identities and Prepared bindings.
   Allocate a fresh destination PublicationNamespace and never-reused backend generation names; copy no process
   locks, source generation counters, routes or credentials as writable authority. This is future import work.
4. Copied pending/cleanup/cursor records are archive evidence only. They authorize no retry, compensation or delete
   against the original installation. Track actual destination effects with its ordinary publication owner.
5. Choose one complete baseline: archived served inventory OR archived desired source/preparation state. Do not mix
   portions. Later desired-state catch-up is normal reconciliation.
6. After real backend readback and verified complete destination current/evidence commit, clear the incomplete marker
   durably. Resolve uncertainty by exact readback under the guard. Only then may normal open/serving begin.

Explicit retry acquires the same private lock and validates incomplete-import state; it does not weaken normal
open or discard populated data. Keep unresolved destination effects bounded under the publication recipe. Do not
create a generic restore journal/state platform. Initialization discard never deletes an incomplete populated import.

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

Collector and archive export are delivered only in their recorded scopes. Next implement fresh-destination import
through the rules above and actual consumers, preserving originals. M1 remains PARTIAL; V1/installed final-head
and whole-product qualification remain NOT_RUN.
