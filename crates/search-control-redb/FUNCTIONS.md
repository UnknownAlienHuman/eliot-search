# Control state and evidence lookup

Source baseline: `c8e38b6`; preparation/publication instructions rechecked at `7c7fd36c` on 2026-10-23.
This selects a replacement mechanism, not a claim that it exists or has passed runtime checks.

## Existing code worth retaining

`persistent/snapshot/lookup.rs::read_current_records_checked` already point-reads selected keys in one
transaction. The generic journal instead appends permanent OPERATIONS, rebuilds all current records into
ControlSnapshot after a mutation and scans receipts during recovery. Normal registry commands add full
v4 batch/original-input replay. Keep typed transactions and point reads; remove those copied databases.

## Use the existing database for compact evidence metadata

The earlier immutable-page proposal did not define efficient evidence lookup. Do not implement another
B-tree, sparse-directory format or whole-manifest memory cache. The pinned redb 2.6.3 supplies ordered
tables, range reads and copy-on-write read snapshots.

For each physical Qdrant generation use two private typed tables with internally derived names:

```text
published_sources[g]: source_id -> exact revision/prepared references and recorded outcomes/gaps
published_evidence[g]: (source_id, point_id) -> identity/input metadata + ObjectRef/representation/range
```

These are metadata, not bodies, vectors or another search index. A source prefix gives its old ordered points;
an exact key resolves a nomination. Qdrant's source ID is only a routing hint: validate selected scope and
exact key/identity in the pinned snapshot. Do not trust payload revision/range or try another source on failure.

ViewManifestEntry means this compact value, not a second authoritative serialized view manifest. Prepared
artifacts hold text/structure/maps; immutable pending pages hold actual unfinished external inputs. Bodies
remain outside redb.

## Snapshot actually held by a view

Private PublishedViewState contains committed route/profile/sequence and an owned redb read transaction.
Open its tables through that transaction. Do not open a fresh read for a later page of an old view or copy
all rows into a BTreeMap.

After commit, the serialized publisher opens a read transaction, verifies the target current-view row,
constructs/installs the state, and only then starts another publication. Queries/windows clone that state.
Old readers retain old bindings when subsequent transactions change the same keys. Keep redb types private.

Long readers retain database pages. Bound request/window lifetime and indexing backlog, and apply visible
write backpressure on exhaustion. Handle counts alone do not bound disk use. Measure retained-page growth;
a snapshot service or durable lease platform is not the remedy.

## Incremental commit versus generation build

For an ordinary update, load/validate the finite metadata delta before opening the writer. After Qdrant
completion/readback, one transaction applies the delta, updates published inventory/current view, inserts
unfinished cleanup and clears pending. No object/RPC I/O inside it. Bound rows/bytes before external effects;
split batches only at complete source boundaries. A source beyond the supported limit is an explicit indexing
gap, never a half-new source silently presented as complete.

Full rebuild uses separate target-generation tables, filled in bounded transactions while the old generation
serves queries. After completion switch the pointer, not all corpus rows in one transaction. The target input
selection also needs a restart rule; filling output tables alone did not supply one.

## Restartable generation input capture

A live ReadTransaction is a process-held snapshot, not a durable application build manifest. Reading page 1,
restarting, then opening current source rows for page 2 can mix source sets/profiles. Saving only a sequence
label or source-key cursor cannot recover the original values. Do not advertise that as resuming fixed input.

For a full/incompatible-profile build, use the existing pending record and target source table:

1. Open one consistent read transaction over current selection, source state and chosen preparation outcomes.
   Retain that same snapshot while copying compact exact input/outcome references into the unserved target
   source table in bounded write transactions. Bodies remain in the immutable store. The publisher owns this
   target and captures the chosen profile/options with it; no Qdrant mutation is dispatched during capture.
2. Only after enumeration and reference validation finish, commit input-capture completion in the existing
   pending record. A partial table is never a complete inventory. The snapshot can then be released.
3. On interruption before completion, abandon/discard that metadata-only partial capture and start a fresh
   capture; no external work exists for it by construction. Do not resume its cursor against a new live read.
4. After completion, all planning, dispatch and recovery use those captured rows and immutable refs. Missing
   input is an explicit failure/gap, not permission to substitute newer source bytes. Build evidence/output
   incrementally without overwriting the frozen selected input with current desired state.
5. Switch only when that captured inventory's required effects and evidence are complete. New source or
   settings changes are discovered by publication's two-sided reconciliation after the switch.

This is full-build-only metadata capture, not a per-update corpus copy, new snapshot database, savepoint
catalogue or receipt protocol. Incomplete capture is owned/referenced by pending until cleaned up. After
external effects are possible, publication's unsettled-dispatch recovery applies instead of dropping target
state. The same original intended inputs must be available for an isolated recovery generation.

The source table must distinguish complete empty/gap outcomes from unfinished processing; a table with zero
points does not establish completed capture or completed build. Capture costs O(selected source metadata)
I/O and retains an MVCC reader while it runs. Bound each transaction/attempt and measure reader growth; it is
not O(page-size) total work and a fresh read per page is not an equivalent optimization.

## Other current state

Settings, root/source heads, prepared outcomes, current view, one pending operation and finite cleanup are
ordinary typed rows. Source state may be ahead of published state. Queries/Exact use the latter and report
actual currency; they do not repair ingestion. Source conditional updates use the [source recipe](../search-source/search-source-registry/FUNCTIONS.md),
not content equality as proof that an old task still owns a row. Resolve uncertain commits from relevant
current rows. No successful read, page or pure transform creates a durable completed-operation record.

## Donor verification

Pinned redb, not a version upgrade:

- [redb 2.6.3 Cargo metadata](https://github.com/cberner/redb/blob/f4e3eb69dc2a4b4e01fefff0b9e039c6c7ab98f6/Cargo.toml).
- [ReadTransaction implementation](https://github.com/cberner/redb/blob/f4e3eb69dc2a4b4e01fefff0b9e039c6c7ab98f6/src/transactions.rs#L2367-L2520): new captures the data root, table reads retain its transaction guard, and close/drop release the process-held transaction. Rechecked in R011.
- [get/range API](https://docs.rs/redb/2.6.3/redb/trait.ReadableTable.html) for point/ordered reads.

Those mechanisms support the design; they do not supply an application restart manifest automatically or
establish latency/Windows durability. R011 used the pinned GitHub source when versioned docs pages were
unavailable; newer redb API pages were not substituted.

## Removal and checks

Remove generic journal/snapshot/registry restore, obsolete grant/key/migration persistence and exclusive
fixtures with callers; do not replace them with a generic repository/versioning framework.

After implementation check exact point lookup, source-prefix updates, old-reader binding stability, atomic
source/evidence/current commit, capture interruption before/after completion, source/profile changes mid-build,
restart from captured refs rather than latest rows, staging isolation, finite transactions and retained-page
growth. Minimal changed-owner Clippy follows code. No new execution result is claimed here.
