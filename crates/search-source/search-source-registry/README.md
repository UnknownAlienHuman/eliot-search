# search-source-registry

Current root, source and current-head metadata for the local application. Retained bytes live in the
immutable object store; published query meaning lives in `PublishedView`. This package must not become a
second corpus, history, migration, operation-replay or access database.

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md), [source map](FUNCTIONS.md) and
[control-state map](../../search-control-redb/FUNCTIONS.md).

## Current production path is a restore pipeline

The native root/source commands do not read a small current registry. They bind `RegistryJournal` through
`bind_original_v4`, restore the full `SourceRegistry` from registry batch history, restore original policy
and admission input layouts, requalify current records, replay or commit another generic batch and then
rebuild/publish the complete control snapshot.

`native_source_command` performs that work before and after the stable source read, yet still outputs:

```text
retained_revision: false
source_body_serving: false
ready: false
```

The normal command therefore pays migration/replay cost without producing a usable retained source.
The small `RegistryJournal::bind` path was found mainly in tests; the normal daemon selects v4 restore.

The pure `registry_snapshot` helper also clones roots, sources, memberships, portfolios, portfolio history,
active generations, ownerships and namespace states into another aggregate before view resolution.
That is not a cheap current-state read and is not needed by the target query path.

## Required current state

Keep only records the running local application uses:

```text
RootRecord
  root_id, selected locator/view, policy, availability

SourceRecord
  source_id, root_id, current locator/view, current_revision, availability

RevisionRef
  revision_id, content_digest, length, immutable_object_ref

optional SavedSelection
  name, ordered source IDs for future requests
```

An immutable object is verified before a short typed redb transaction changes `current_revision`.
Replaying the same source/head and revision is harmless; the same expected head with another revision
conflicts. Source removal affects future views but does not delete retained objects still referenced by a
current, pending or live published view.

Publication builds the immutable `PublishedView` used by queries. That view binds exact source revisions,
route/profile and evidence manifest. The registry needs no permanent historical SourceView/corpus/
portfolio revisions or access/scoring proof records to make one coherent query.

## Small operations

```text
register/update/unbind one root
get/list one bounded page of current roots/sources
commit_source_head(source, expected_head, RevisionRef, locator, availability)
retire_source_head(source, expected_head, reason)
read one consistent current source snapshot for reconcile/publication
resolve an explicit/default source subset without widening
```

Use typed redb rows and exact point/bounded-page reads. A read-only in-memory cache may mirror current rows
while the daemon owns the root, but it is not reconstructed from operation history before every command and
is never an outage fallback.

A source/head commit stores no source body, admission transcript, original input layout, canonical batch or
completed operation receipt. A genuinely uncertain redb commit is resolved by reading the current source
row. The immutable object may be left unreferenced for bounded GC if the head commit did not occur.

## Remove with callers

- `RegistryJournal` adapter, operation envelopes and full batch restore on normal commands;
- v4 original policy/admission retention and reconstitution;
- package-wide `SourceRegistry` replay before one root/source mutation;
- generic namespace owner cutover, portfolio/history and membership policy machinery without a current
  user-facing consumer;
- legacy DIRECT/root-catalog authority and receipt-bound currentness;
- `search-source-records` / `search-source-history` forwarding splits once useful current types move to
  their actual owner;
- process-local operation/receipt collections and full aggregate snapshot cloning.

Keep read-only legacy code only while an explicitly selected old-format diagnostic needs it. It cannot
write current state or block a clean current-format application.

Preserve #598's active private source-retention work and consume its accepted retained revision/head result;
do not create a competing writer. Unsupported pre-release roots may be rejected before writes and left
intact.
