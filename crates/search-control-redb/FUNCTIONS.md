# Control state and evidence lookup

Source inspected at `c8e38b6`; replacement design revised 2026-10-21. This is an implementation decision,
not a claim that the replacement exists or has passed runtime checks.

## Existing code worth retaining

`persistent/snapshot/lookup.rs::read_current_records_checked` already point-reads selected keys in one
transaction. The generic journal instead appends permanent `OPERATIONS`, rebuilds every current record into
`ControlSnapshot` after a mutation and scans receipts during recovery. Normal registry commands add full
v4 batch/original-input replay. Keep typed transactions and point reads; remove those copied databases.

## Use the existing database for compact evidence metadata

The previous proposal moved all evidence bindings to immutable pages without defining their lookup index.
Do not implement a second B-tree, sparse-directory format or whole-manifest memory cache. The pinned
`redb = 2.6.3` already supplies ordered tables, range reads and copy-on-write read snapshots.

For each physical Qdrant generation use two private typed tables, with names derived internally from its ID:

```text
published_sources[g]: source_id -> exact revision/prepared references and recorded gaps
published_evidence[g]: (source_id, point_id) -> identity/input metadata + ObjectRef/representation/range
```

These are metadata rows, not source bodies, vectors or an additional search index. One source prefix gives
its old ordered points for update planning; one exact key resolves a nomination. Qdrant's source ID is only
a routing hint: it must belong to the selected scope, and the exact key/identity must exist in the pinned
snapshot. Never trust the payload's revision/range or search another source after a failed lookup.

`ViewManifestEntry` now means this compact binding value. There is no second authoritative serialized view
manifest. Prepared artifacts retain their useful text/structure/maps; immutable publication pages retain
inputs for unfinished effects. Their bodies remain outside redb.

## Snapshot actually held by a view

A private `PublishedViewState` contains the committed route/profile/sequence and an owned redb read snapshot.
Open its source/evidence tables through that snapshot. Do not begin a fresh database transaction for a later
page of an old view, or copy the whole table into a `BTreeMap`.

After publication commits, the serialized publisher opens a read transaction, verifies that its current-view
row is the target, constructs the state, and installs it before starting another publication. A new query
clones this state; a session window inherits it. Old readers retain old bindings through redb MVCC even when
a later transaction changes the same keys. Redb types stay private to the control/publication composition.

Long-lived readers retain database pages. This is a real disk-retention cost, not free snapshotting: bound
request/window lifetime and indexing backlog, and stop accepting additional updates on resource exhaustion.
Do not promise bounded disk use solely from the count of view handles. Measure with a stalled publisher and
an old reader; a new snapshot service or durable lease is not the remedy.

## Incremental commit versus generation build

For an ordinary update, load and validate the finite metadata delta before opening the writer. After Qdrant
readback, one transaction applies that delta, updates published source inventory, changes current view,
inserts unfinished cleanup and clears pending. No object or RPC I/O runs inside it. Bound delta rows/bytes
before any external effect; split a multi-source batch only at complete source boundaries. An individual
source beyond the supported limit produces an explicit indexing gap, not a half-new published source.

A full rebuild must not become one corpus-sized transaction. Fill separate target-generation tables in
bounded transactions while the old generation remains current. Pending state owns the target generation and
its exact selected inputs. Once complete, publish by changing the current-generation pointer, not copying
all rows. Current queries never open staging tables. Source changes after the selected build snapshot are
queued for a following update rather than substituted silently.

## Other current state

Settings, root/source heads, current prepared refs, current view, one pending publication and finite cleanup
remain ordinary typed rows. Current source heads may be newer than published inventory; query and Exact use
the latter. A known abort changes nothing; uncertain commit is resolved by reading the relevant typed rows.
No successful read, page or pure transformation creates a durable operation record.

## Donor verification

Checked the repository-pinned version, not latest redb:

- [redb 2.6.3](https://docs.rs/crate/redb/2.6.3): copy-on-write B-trees and concurrent MVCC readers.
- [get/range](https://docs.rs/redb/2.6.3/redb/trait.ReadableTable.html): point lookup and ordered iteration.
- [ReadTransaction source](https://github.com/cberner/redb/blob/f4e3eb69dc2a4b4e01fefff0b9e039c6c7ab98f6/src/transactions.rs): captures a table-tree root and retains a transaction guard when opening tables.

This supports the selected mechanism, not a measured latency or Windows durability claim. No dependency
upgrade is required by this design.

## Removal and checks

Delete generic journal/snapshot/registry restore, obsolete grant/key/migration persistence and their exclusive
fixtures after callers move. Do not replace them with another generic repository or versioned-record framework.

Check through real callers: point lookup without corpus enumeration; source-prefix update; old reader after
binding replacement; current/source/evidence atomicity; restart before/after commit; staging-generation isolation;
bounded final delta; and disk growth while old snapshots are held. Minimal changed-owner Clippy follows code.
