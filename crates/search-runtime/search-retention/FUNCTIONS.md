# Stopped-root maintenance and plain archives

This recipe owns local-object reachability, archive and import. [Publication](../../search-index-qdrant/search-publication/FUNCTIONS.md)
owns live Qdrant retirement. [Local scope](../../../docs/architecture/LOCAL_SCOPE.md) selects plain storage, not key migration.
Main still contains the old retention/restore code; #610's removal has not landed. These are replacement instructions.

## Historical findings and remaining legacy code

The legacy DIRECT collector rebuilds references from the whole source-events log, retaining historical rows while
missing the target preparation/published/pending owners. Query-time refresh_storage repeats storage scans.
Do not reuse this reachability algorithm. [Earlier findings](https://github.com/UnknownAlienHuman/eliot-search/blob/6ae24197a1be8695ede5bb7d12dfaf97f5bca1fc/crates/search-runtime/search-retention/FUNCTIONS.md)
retain exact source seams; recheck callers at the cut. Removing old code does not implement maintenance.

## No maintenance scan in the query path

Query reads only selected evidence and reports actual owner state. Do not recursively audit objects or replay logs
just to print a status/page. Full inventory is an explicit diagnostic/maintenance action, not a runtime credential.
Disk pressure pauses writes and reports maintenance; it does not authorize online local-object deletion.

## Stopped-root local-object collection

Stop the daemon and all application readers/writers; acquire the same exclusive root guard. Busy means refuse,
not kill another process. Open one supported consistent control snapshot before marking.

### One complete mark

Enumerate current source/retained revisions, preparation outcomes/artifacts, every retained published generation,
fixed build input/pending pages, unresolved cleanup references and any supported incomplete import. No process-local
window remains under stopped-root exclusion. Do not reconstruct liveness from operation history or supplied manifests.

Validate all typed tables/references; missing/corrupt/unknown/unreadable or over-limit input stops before deletion.
Mark exact ObjectRefs once in disposable bounded scratch data outside final objects. Raw bodies are leaves.
Complete the entire mark before sweep. An incomplete mark does not authorize deletion of an apparently safe subset.

### One sweep

Scan final objects once. Delete only known store-owned current-format objects absent from the complete mark;
preserve foreign files, unsupported formats, uncertain identities and required sole copies. Owned temporary stages
may be removed only when no producer exists. Before-sweep interruption deletes no final object. During-sweep failure
may leave garbage; the next run rebuilds marks. No sweep journal, legal-hold or erasure-proof platform.

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

## Explicit restore/import into a fresh destination

1. Read the complete archive without writes or original backend effects. Validate format, references, exact objects,
   representation/gaps and bounded paths. Missing/unsupported/corrupt input fails, not rereads of changed live files.
2. Under the destination's exclusive root guard, initialize an empty plain root. Persist a destination-bound
   incomplete-import marker before importing rows/objects or issuing RPCs. Normal open refuses it before recovery.
3. Use the destination object owner for exact bytes. Allocate new destination identity and never-reused generation
   names; copy no process locks, source generation counters, routes or credentials as writable authority.
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

## Remaining implementation and verification

Replace actual legacy GC/restore/status consumers and remove displaced source/Cargo/fixtures together. Keep the
root guard, object identity/path checks and preserved originals. Minimal compiler/Clippy follows production code.
Later cases cover busy refusal, all mark roots, corruption, interruption, partial/marker-stripped archive refusal,
incomplete-import startup, original/destination sharing an endpoint without original mutations and plain restart.
No runtime or deletion result is claimed by this document.
