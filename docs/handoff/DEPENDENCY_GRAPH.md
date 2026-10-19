# Current implementation dependency graph

The product is one application with concrete owners. Package boundaries may shrink or disappear; this graph
records effects and data ownership, not a permanent crate topology.

```text
eliot-search CLI
  → one framed local client
  → eliot-searchd direct dispatcher

eliot-searchd
  ├→ source owner
  │    ├→ safe final-handle/Git read
  │    ├→ one immutable object writer/reader
  │    └→ typed redb RootRecord / SourceRecord / RevisionRef
  ├→ preparation owner
  │    └→ materialization → structure-first units → retrieval inputs
  ├→ publication owner
  │    ├→ immutable operation/manifest/cleanup pages
  │    ├→ typed redb PendingPublication / CurrentPublishedView / RetiredCleanup
  │    ├→ one real Qdrant runtime
  │    └→ one private current-view manager
  └→ query owner
       ├→ current ViewGuard
       ├→ non-widening ResolvedScope
       ├→ RequestContext(deadline, cancellation, counters)
       ├→ real Qdrant query or explicit retained-source exact scan
       ├→ ViewManifestEntry → immutable object/range evidence
       └→ one process-local QueryWindow
```

## Ownership rules

- Source bytes have one immutable object owner. redb stores references, not duplicate bodies.
- Qdrant is a derived retrieval index. It never owns source evidence or durable publication recovery.
- `PublishedView` is the immutable query snapshot. Source registry/current control state is not a second query
  snapshot.
- Publication is the only Qdrant mutation/recovery/current-view owner.
- Query reads have no durable operation journal.
- Optional restricted clients, model providers, overlays, connectors, orientation and comparison exist only
  after a real consumer/implementation is selected. They narrow or enrich this graph; they do not create a
  parallel application.

## Interfaces

Use concrete owner methods. A narrow trait is local to a consumer only when multiple real implementations
exist. There is no global `search-ports` registry, service locator, effect bus or conformance catalogue.

```text
one implementation        → concrete type/method
multiple current adapters → smallest consumer-owned trait
test fake only             → private/local fake, not public product architecture
```

Cross-package shared types live in `search-contracts` only when at least two real current owners use them.
Vendor/redb/OS types stay private to their adapter where useful.

## Cutover order

```text
1. current root/source/object/control path
2. one preparation and stable retrieval/evidence model
3. real Qdrant publication + current view
4. real source-backed Search
5. one framed CLI path and QueryWindow
6. exact scan, rebuild and bounded cleanup/GC
7. optional features only from measured user need
```

Every slice connects the real producer and consumer and removes the replaced mechanism. A trait, adapter,
receipt, capability, issue or test fixture is not delivery.

The former Architecture 8.4 graph at commit `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`
is historical evidence only. It cannot restore the global port catalogue, wave dependencies or pre-release
compatibility requirements.
