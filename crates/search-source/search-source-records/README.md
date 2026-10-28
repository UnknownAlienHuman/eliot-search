# search-source-records — legacy schema/codec bundle scheduled for deletion

This package owns no I/O or current source truth. It is a relocation bundle for pre-release registry,
DIRECT, batch, snapshot, portfolio, namespace and provenance records.

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md),
[source registry map](../search-source-registry/FUNCTIONS.md) and
[admission map](../search-source-admission/FUNCTIONS.md).

## Verified source inventory

Production files total about **332 KiB** before package tests:

```text
batch / batch codecs / v4 codec        ~99.5 KiB
snapshot + canonical snapshot           ~45.5 KiB
operation/error                          ~26.6 KiB
legacy DIRECT/root catalog               ~34.3 KiB
namespace/membership/portfolio/history   ~52.9 KiB
root/source/currentness                  ~37.1 KiB
identity/source-request/provenance       ~30.3 KiB
other                                    ~5.8 KiB
```

The main consumers are `search-source-registry`, `search-source-history` and daemon registry restore. They
reexport these types and replay batch/history/original-input state. The package is not a neutral shared model;
it is the old registry design split into another crate.

## Current-format values

The new application needs only small typed current state near its actual owners:

```text
RootRecord {
  root_id
  selected locator/view and policy revision
  availability
}

SourceRecord {
  source_id
  root_id
  current locator/view
  current RevisionRef
  availability
  optional current admission outcome/reason/classification
}

RevisionRef {
  revision_id / content identity
  byte length
  immutable ObjectRef
}

optional SavedSelection {
  name
  ordered source IDs
}
```

`PublishedView` owns immutable query membership/evidence. The source registry does not retain permanent
portfolio, membership, currentness or operation histories to keep old queries coherent.

Concrete redb row codecs belong to `search-control-redb`/source composition. Immutable object identity
belongs to the object store. Do not create provider-neutral registry records solely to avoid changing those
owners.

## Delete after direct current-state cutover

- generic batch and v4 batch codecs;
- complete registry snapshot/canonical snapshot;
- operation envelopes/receipts and restore errors;
- legacy DIRECT and root-catalog formats;
- namespace owner cutover and membership/portfolio history;
- historical root currentness and source admission receipts;
- original-input/provenance/source-request v2 records;
- forwarding reexports in `search-source-registry` and `search-source-history`;
- tests/goldens/status/ledger entries protecting those pre-release bytes.

Move only the few current value fields above to their concrete source/control owner. No forwarding crate,
`records-v2` or compatibility facade remains.

Unsupported old roots are refused before writes and left intact. Preserve #598's active private work and
consume only its accepted retained revision/head result.

## Completion

Normal startup point-reads current root/source/head rows directly from redb. One source update writes one
immutable object and one typed head transaction. Inventory/publication may iterate bounded current rows,
but no command restores batch history, snapshots, portfolios or original admission inputs.
