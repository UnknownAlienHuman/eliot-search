# search-source-registry — collapse to typed current rows

Current root, source and source-head state belongs in short typed redb operations. Retained bytes live in the
immutable object store; immutable query membership/evidence lives in `PublishedView`. The registry is not a
second database, replay engine, corpus, access authority or history.

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md), [source map](FUNCTIONS.md),
[control-state map](../../search-control-redb/FUNCTIONS.md) and
[current identity map](../search-source-identity/FUNCTIONS.md).

## Verified current source

Production files total about **214.4 KiB** before more than 137 KiB of inline/external tests:

```text
api / batch / recovery / restore       ~83.8 KiB
cutover / membership / portfolio       ~37.2 KiB
root / source / source-v2 / view       ~69.1 KiB
error / lib / IDs                      ~21.3 KiB
```

The normal daemon does not read a small current registry. It binds `RegistryJournal` through v4 original-
input restore, reconstructs complete `SourceRegistry`, replays a batch/operation, mutates one item, reads
batch targets back and rebuilds a complete control snapshot. Source admission still returns no retained
revision.

`registry_snapshot` separately clones roots, sources, memberships, portfolios, portfolio history, active
generations, namespace ownership and namespace state before resolving a view.

## Current records

```text
RootRecord {
  RootId
  current locator/view and policy revision
  availability
}

SourceRecord {
  SourceId
  RootId
  current locator
  optional current physical key
  current RevisionRef
  availability
  optional current admission outcome/reason/classification
}

RevisionRef {
  revision/content identity
  byte length
  immutable ObjectRef
}

optional SavedSelection {
  name
  ordered SourceId[]
}
```

The root/source/control owner may store these directly in typed redb tables. A separate in-memory
`SourceRegistry` is optional only as a read-through cache while the daemon owns the root; it is never rebuilt
from operation history before every command and never becomes durable authority.

## Current operations

```text
register / update / unbind root
get root/source and bounded list pages
commit_source_head(source, expected_head, RevisionRef, locator, physical_key, availability)
retire_source_head(source, expected_head, reason)
read one consistent current source snapshot for inventory/publication
resolve explicit/default selected SourceId subset without widening
```

The immutable object exists and is read back before the short source-head transaction. A known abort is a
known failure. A genuinely uncertain commit is resolved by point-reading the source row. Failure before the
head leaves a safe unreferenced object for bounded GC.

Source identity matching is current-state logic: same locator, optional proven rename by exact physical key,
new source or explicit conflict. No permanent path/lineage history.

## Remove with callers

- package-wide `SourceRegistry` restore/replay and generic operations;
- batch, source-v2, recovery and registry-restore paths;
- namespace owner cutover and membership/portfolio/history machinery;
- cloned aggregate `registry_snapshot` and old query views;
- receipt/currentness/original-input/provenance fields;
- `search-source-records` and `search-source-history` forwarding packages;
- old identity binding/history/lineage framework;
- legacy DIRECT/root-catalog authority and fixtures.

If the remaining operations are naturally methods of typed redb/source composition, delete this crate rather
than retain another repository layer.

Unsupported pre-release roots fail before writes and remain intact. Preserve #598's active private work and
consume only its accepted retained revision/head result.

## Completion

One acquired body becomes one immutable object and one current source head. Restart point-reads current rows
without operation replay. Inventory/publication obtains one consistent bounded current snapshot. Old query
evidence remains exact through its pinned `PublishedView`, not registry history.
