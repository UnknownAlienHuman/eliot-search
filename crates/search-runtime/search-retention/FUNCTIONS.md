# Local maintenance, backup and restore/import

This file owns offline local-object collection and current-format archive/import. Qdrant retirement remains
[publication cleanup](../../search-index-qdrant/search-publication/FUNCTIONS.md). This is now a documentation-only
location, not a Cargo package. The selected operations below are not implemented by removing their predecessors.

## Historical findings and remaining legacy code

At `2b37f5ef`, `search-retention` contained generic leases, purge planes, graph sweep, receipts and restore phases.
The daemon's `restore_composition` required the destination root/incarnation/epoch and physical root to match the
export, carried copied collection/purge/key authority and supported sole-copy deletion. R017 removed both
implementations; R020 removes the empty crate and its Cargo/lock/feature entries. Do not adapt or reconstruct them.

The legacy DIRECT collector remains a separate removal target. `collect_orphan_revisions`:

1. reads the complete append-only `source-events.log` into memory;
2. adds the revision from every historical event to `referenced_revisions`, including retired/superseded rows;
3. scans only the legacy `revisions/` tree;
4. knows nothing about current preparation, published evidence, pending publication or cleanup references.

It therefore preserves every revision ever logged while still missing the target owners. It can remove only
unlogged legacy or temporary files. Its preview/apply wrapper repeats the full scan; a later storage-status
refresh scans again. Preserve useful path/symlink/name checks, not this reference algorithm.

## No maintenance scan in the query path

The inspected DIRECT query/page/continue/expand commands call `refresh_storage` before output.
`StorageSecurityStatus::inspect` then runs the legacy GC inventory, scans revision objects again for protected
headers and scans preparation objects. One interactive page can therefore replay up to the full control log and
walk revision/preparation trees merely to print storage metadata.

Remove that dependency. Normal startup/query/output may report the selected storage mode and verify each
actually read object through its owner. It must not claim that every object is encrypted or complete from file
suffixes/magic, and must not recursively audit storage per result. A separate explicit diagnostic/maintenance
command may perform a full inventory and report its cost/snapshot. That report is not a runtime credential.

## Stopped-root local-object collection

Collection is an explicit command after the daemon and all writers/readers have stopped. Acquire the actual
exclusive root guard. Busy means refuse; never kill the owner or run a second store instance.

### One complete mark

Open one consistent current control snapshot and validate its supported schema before deletion. Enumerate all
persisted roots that can require final local objects:

- current source heads and retained revisions;
- current preparation outcomes, including successful empty/gap metadata and their artifacts;
- every published generation still retained by current state;
- staging/build input tables and one pending publication/effect artifact;
- unfinished cleanup/recovery obligations that still reference local artifacts;
- an incomplete import owned by this destination, if the selected implementation can leave one.

There are no live process-local views/windows under stopped-root exclusion. Do not reconstruct roots from
completed-operation history, append-only source events, leases or caller-provided manifests.

Traverse only known typed metadata. Fail before sweep if a supported current table/reference encoding is
missing, corrupt, truncated, unknown to the collector, exceeds finite limits or cannot be read. Unknown metadata
is not permission to delete an "unaffected" subset.

Mark exact `ObjectRef`s once in disposable scratch redb data outside the final object namespace. Use finite
cache/pages/batches; scratch size follows distinct references. Raw bodies are leaves. Finish and validate the
whole mark before any final-object deletion.

### One sweep

Enumerate the final-object namespace once. Delete only store-owned current-format identities absent from the
completed mark. Preserve foreign names, unsupported formats, uncertain identities and sole retained copies.
Store-owned temporary stages may be removed because no producer exists under the exclusive guard.

Interruption before sweep deletes nothing. Interruption during sweep can leave garbage but cannot make a marked
object eligible. The next run discards old scratch and rebuilds marks; no sweep journal/history is needed.
Deletion/readback uncertainty preserves the object or reports failure. Do not repeat the full mark merely to
emit status metrics.

Disk pressure while serving pauses new writes and reports maintenance. It does not authorize online local GC.
Qdrant cleanup is separate online work constrained by readers and possible late writers.

## Backup is a completed archive, not a runnable clone

Stop the source daemon, close database writers/readers and hold its exclusive root guard. Create a new staging
archive outside the source root. Copy only supported committed control data and required final objects:

- current source/preparation state;
- the last committed published inventory/evidence needed to reproduce the served baseline;
- pending/cleanup records and their referenced objects as non-executable evidence of unresolved source state;
- format/storage profile metadata required to read the copied objects.

Qdrant collections, process locks, sockets, scratch marks, caches and store-owned transient stages are not
portable authority and are excluded by default. If unresolved state references a final artifact, copy that
artifact even though the destination will not execute the old operation.

Verify the copied control snapshot and every referenced copied object from the staging archive itself. Sync
copied files/directories where the selected platform provides a real durability primitive. Only after complete
verification write the small archive-complete marker last (or atomically publish the staged directory). A
partial directory has no complete marker and is never accepted as backup.

The archive marker names the supported archive/root format and source installation for diagnostics; it is not
a second object graph or receipt system. Normal daemon open checks/refuses the marker before any external effect.
A raw OS clone lacking this supported archive protocol remains unsafe and is not automatically adopted.
Keep the source and archive intact.

## Explicit restore/import into a fresh destination

1. Open the complete archive read-only. Validate format, source/prepared/published references, object identities,
   gaps and required storage material. Missing/unsupported data leaves the destination uncommitted.
2. Initialize an empty destination through the normal root owner. Allocate new installation/control identity and
   a fresh never-conflicting Qdrant generation. Do not copy process locks, owner epochs or generation counters.
3. Import exact retained source/prepared objects through the destination object owner and create destination-
   local current rows. Never reread changed live paths as a substitute for archive bytes.
4. Treat copied pending/cleanup/routes/cursors as archive evidence only. They authorize no RPC, compensation,
   deletion or retry against the original endpoint.
5. Build one coherent destination baseline from either:
   - the fully validated archived published inventory, preserving the last served view before later catch-up; or
   - the fully validated desired current source/preparation inventory.
   Do not mix portions of both. Newer desired state remains a following ordinary reconciliation.
6. Install the destination view only after real Qdrant readback and one destination-local metadata commit.
   Destination cleanup may name only generations created by this destination.

Failed/interrupted import leaves an explicitly incomplete destination that normal serving refuses. Retry may
restart from validated archive data; no generic restore state machine, quarantine generation or copied authority
is required. Restore never deletes the archive/source automatically.

## Protected storage

Plain storage needs no key platform. Readability of copied ciphertext does not establish safe writable
continuity. Never resume/reset a copied deterministic nonce counter under the same actual key. Archived objects
may be read only through the original supported binding. New destination writes require the accepted storage
owner to create an independent real key/nonce context. If unavailable, report writable restore unsupported and
leave the archive unchanged; no silent plaintext downgrade, envelope relabel, generic re-encryption or key-
migration receipt system.

## Remaining implementation and verification

The generic package, daemon restore composition and exclusive lifecycle fixtures are removed. Replace the still-
existing legacy DIRECT GC/status hot-path consumers with concrete maintenance/import operations; do not revive
the removed package. Keep useful filesystem safety checks and causal cases, not old types.

After implementation and minimal pinned compiler/Clippy, check:

- busy-root refusal and no query-time recursive inventory;
- complete typed-root coverage, superseded revision reclamation and current/published/pending roots;
- malformed/unknown metadata, scratch exhaustion, cancellation and crash before/during sweep;
- foreign files and store-owned temporary cleanup;
- archive marker written last, partial-copy refusal and copied-object verification;
- original and restored installations sharing one Qdrant endpoint with zero original mutations/deletes;
- archived-published baseline versus newer current state, interrupted import and distinct destination generation;
- missing secret and protected writable-copy refusal.

These are required causal cases, not executed results.
