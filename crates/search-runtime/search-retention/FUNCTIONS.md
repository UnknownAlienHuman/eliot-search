# Local object collection: one stopped-root pass

The old package's lib/sweep/purge modules implement leases, caller graphs, operation receipts, purge and
restore machinery. They are not retained. This file selects the smaller replacement for local immutable
objects; Qdrant deletion remains the publisher's separate unfinished external work.

## Defect in the replacement proposal

The previous recipe rescanned every live manifest while holding an exclusive GC gate before each deletion
page. For R reachable references and P candidate pages that can repeat O(P * R) traversal. A small deletion
page does not bound the preceding root traversal or its writer pause. Building publication artifacts before
their first reference also left ambiguous how long producers must hold the shared gate.

Do not repair this by adding barriers, reference-count journals or a generic incremental graph collector.
The first local collector is an explicit maintenance command on a stopped current-format root.

## Exclusion and lifetime

Acquire the same actual exclusive root guard as the daemon. If busy, refuse without modifying the root. Do
not force-stop another process. No producer, query window or live old-view reader exists during the pass;
restart has already ended process-local windows. Keep the guard for the whole command, not one page.

While Search is running, successful writes still remove their own stages, but final immutable objects are
not automatically garbage-collected. Disk pressure stops new ingestion/publication safely and reports the
need for maintenance; it does not evict evidence. This is an explicit availability trade-off, not a claim of
online collection. A future online collector requires a measured need and a separately justified algorithm.

## Mark once, delete only after complete traversal

1. Open one consistent current control snapshot. Enumerate current source/prepared refs, the current
   generation's published source/evidence refs, pending operation refs and unfinished cleanup refs.
2. Visit outgoing references only in known typed preparation/operation/cleanup metadata. Mark raw source
   bodies as leaves; do not read/hash every source merely to determine reachability.
3. Insert each exact ObjectRef into a disposable scratch redb table. Use the existing library's finite
   cache and bounded batches instead of an unbounded HashSet or a new sorting/index implementation.
4. Complete and validate the entire root traversal before deleting any final object. Missing, corrupt,
   unsupported or truncated reference metadata aborts collection with zero final-object deletions. There is
   no guessed 'affected set' unless a real independent namespace has been proved.
5. Enumerate finalized object names once in bounded pages. Check each exact identity against the mark table;
   delete only recognized current-format unmarked objects through the object owner. Leave foreign names,
   unknown layouts and uncertain deletions intact.
6. Close/delete scratch data and release the root guard. Never persist a completed-GC receipt or resume from
   an old mark set. After any interruption, the next run rebuilds marks from its newly locked root.

Scratch redb is temporary membership data, not a second product database or authority. Never let its files
enter the final object namespace. A failed marking write, limit or cancellation before sweep deletes
nothing. Interruption during sweep may leave garbage, but no live object was selected for deletion.

Traversal is once per command, plus ordered enumeration and indexed mark lookups. Memory follows configured
cache/page limits; temporary disk scales with distinct reachable ObjectRefs. State that disk cost and fail
cleanly when it cannot be met. No performance result is claimed before measurement.

## Stages and pending work

A stage is removable only when recognized as this store's temporary format, never because an arbitrary file
is old. With the daemon absent no active writer owns one; finalization-success cleanup remains immediate.
Pending and cleanup artifacts are roots even if Qdrant is unavailable. Maintenance never runs their effects,
clears pending, or turns a copied object into a published source. An incomplete pending reference list aborts
collection instead of guessing which pages are disposable.

## Other lifecycle operations

Source removal affects new published views; while serving, old guards continue to reopen old evidence.
Stopped-root collection later removes unreferenced local objects. This is not legal/security purge or a
physical-erasure promise. Backup remains an explicit coherent stopped-root copy with retained pending state;
Qdrant is rebuilt as derived data, not used as the backup authority.

## Remove and verify

Remove search-retention, wave7-lifecycle, restore/purge/lease models, the proposed ObjectGcGate and their
exclusive adapters/fixtures. Keep exact object enumeration/deletion with the actual immutable store owner.

After implementation check busy-root refusal, first-reference interleaving impossible under exclusion,
current/published/pending/cleanup coverage, malformed metadata before deletion, cancellation/crash, scratch
restart invalidation, foreign files and eventually reclaimed orphans. Verify total traversal work, not only
size of one deletion page. Minimal changed-owner Clippy follows code.
