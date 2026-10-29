# Local maintenance, backup and restore/import

This file owns offline local-object collection and current-format archive/import. Qdrant retirement remains
[publication cleanup](../../search-index-qdrant/search-publication/FUNCTIONS.md). These are target operations,
not implemented capabilities or a reason to retain the old generic package.

## Historical findings and remaining legacy code

At `2b37f5ef`, `search-retention` contained generic leases, purge planes, graph sweep, receipts and restore phases.
The daemon's `restore_composition` required the destination root/incarnation/epoch and physical root to match the
export, carried copied collection/purge/key authority and supported sole-copy deletion. R017/R020 removed that
machinery on the unmerged refactor branch only. At main `623160a7`, Cargo still includes `search-retention`.
Inspect current callers and remove the displaced platform with them; do not infer deletion from this recipe.

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
archive outside the source root. From the first write, use a distinct archive layout: archived control data and
objects are data for the importer, never stores opened by the normal daemon. Do not copy a runnable root manifest
or place the root-format signature at any normal-open location. Missing completion metadata must leave an invalid
archive, not an apparently ordinary installation. Copy only supported committed data and required final objects:

- current source/preparation state;
- the last committed published inventory/evidence needed to reproduce the served baseline;
- pending/cleanup records and their referenced objects as non-executable evidence of unresolved source state;
- format/storage profile metadata required to read the copied objects.

Qdrant collections, runnable root manifests, process locks, sockets, scratch marks, caches and store-owned
transient stages are excluded. Record needed format/profile information as archive metadata, not a live-root
identity. If unresolved state references a final artifact, copy it without executing its old operation.

Verify the copied control snapshot and every referenced copied object from the staging archive itself. Sync
copied files/directories where the selected platform provides a real durability primitive. Only after complete
verification write the small archive-complete marker last (or atomically publish the staged directory). A
partial directory has no complete marker and is never accepted as backup.

The completion marker names the archive format and source installation for diagnostics; it is not a runnable
root manifest, second graph or receipt system. Ordinary open refuses complete AND incomplete archive layouts
before replaying pending/cleanup state or contacting Qdrant. Removing the marker must not turn an archive into a
valid root. A raw OS clone is outside this protocol and is not automatically adopted. Keep source and archive intact.

## Explicit restore/import into a fresh destination

1. Open the complete archive read-only. Validate format, source/prepared/published references, object identities,
   gaps and required storage material. Missing/unsupported data leaves the destination uncommitted.
2. Initialize an empty destination with new root identity and keep its exclusive guard. Before importing rows,
   objects or issuing backend effects, durably set the current-format incomplete-import marker defined by the
   [root owner](../search-runtime-owner/FUNCTIONS.md#archive-and-import-boundary). Allocate fresh destination-only
   collection names; do not copy process locks, owner epochs or generation counters.
3. Import exact retained source/prepared objects through the destination object owner and create destination-
   local current rows. Never reread changed live paths as a substitute for archive bytes.
4. Treat copied pending/cleanup/routes/cursors as archive evidence only. They authorize no RPC, compensation,
   deletion or retry against the original endpoint.
5. Build one coherent destination baseline from either:
   - the fully validated archived published inventory, preserving the last served view before later catch-up; or
   - the fully validated desired current source/preparation inventory.
   Do not mix portions of both. Newer desired state remains a following ordinary reconciliation.
6. After real backend readiness, commit complete destination-local current/evidence state. Resolve uncertain
   commits by readback before clearing the import marker durably; then install the view. Destination cleanup
   may name only generations created by this destination.

Ordinary open of an incomplete import refuses before backend recovery/cleanup, not merely before query serving.
A crash before the marker is installed leaves only a newly initialized empty root; no imported rows or RPCs exist.
A crash after it is cleared uses ordinary committed-view startup. Explicit import retry revalidates the archive
and destination, retaining any unresolved destination-only effects under the
[publication recovery rule](../../search-index-qdrant/search-publication/FUNCTIONS.md#small-conservative-recovery-boundary).
One incomplete marker is enough: no phase journal or copied authority. Never use empty-init discard for a populated
import, or delete source/archive automatically. Implement this boundary with import in Stage 5, not as a Foundation gate.

## Protected storage

Plain storage needs no key platform. Readability of copied ciphertext does not establish safe writable
continuity. Never resume/reset a copied deterministic nonce counter under the same actual key. Archived objects
may be read only through the original supported binding. New destination writes require the accepted storage
owner to create an independent real key/nonce context. If unavailable, report writable restore unsupported and
leave the archive unchanged; no silent plaintext downgrade, envelope relabel, generic re-encryption or key-
migration receipt system.

## Remaining implementation and verification

Replace legacy retention/restore and DIRECT GC/status consumers with concrete maintenance/import operations,
then remove their displaced APIs and exclusive fixtures. Main and the unmerged branch have different removal
status; use the actual checked-out callers. Keep filesystem safety checks, not obsolete public types.

After implementation and minimal pinned compiler/Clippy, check:

- busy-root refusal and no query-time recursive inventory;
- complete typed-root coverage, superseded revision reclamation and current/published/pending roots;
- malformed/unknown metadata, scratch exhaustion, cancellation and crash before/during sweep;
- foreign files and store-owned temporary cleanup;
- complete/partial/marker-stripped archives refused by normal open before any RPC; copied-object verification;
- original and restored installations sharing one Qdrant endpoint with zero original mutations/deletes;
- archived-published versus desired baseline; crashes before/after import marker and current commit;
- ordinary open refusing an incomplete import before recovery; distinct destination generation;
- missing secret and protected writable-copy refusal.

These are required causal cases, not executed results.
