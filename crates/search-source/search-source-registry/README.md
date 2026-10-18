# search-source-registry

Current source metadata for the local application. Retained bytes live in the revision store; published
query meaning lives in `PublishedView`. This package must not become a second corpus, history, migration or
access database.

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and [source map](FUNCTIONS.md).

## Current source reality

The daemon's native Source-v2 command uses `prepare_source_admission_v2` /
`commit_source_admission_v2` to persist admission metadata. It does not retain the acquired body or commit
a usable revision/head.

The package also exposes roots, memberships, reference portfolios, historical views, owner cutover,
batch/restore and receipt-oriented operations. Public-main searches found many of those entry points only
in package fixtures or adapter code. `register_source_revision` likewise has test callers, not the native
retention path. A large logical API is not a connected source catalogue.

`search-source-records` and `search-source-history` are largely relocations/reexports of legacy codecs,
currentness and replay code. They are current code locations, not independent product authorities.

## Required current state

Keep the records the running application needs:

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

An immutable object is published before a short control transaction changes `current_revision`. Replaying
the same source/revision is harmless; a conflicting object/reference fails. Source removal changes current
availability and future views but does not erase retained bytes still referenced by a current, pending or
live published view.

Publication builds the immutable `PublishedView` used by queries. That view binds selected source
revisions, route/profile and evidence manifest. The source registry does not need permanent historical
`SourceView`/corpus/portfolio revisions or access/scoring proof records to make one coherent query.

## Small operations

- register/update/unbind one selected root;
- commit or retire one current source head after retained-object readback;
- list current roots/sources and obtain a consistent snapshot for ingestion/publication;
- resolve an explicit/default source subset without widening;
- keep actual unfinished source/control mutation state only where commit outcome is uncertain.

Use direct typed redb operations in the existing control owner. Do not retain an eternal operation receipt
history or an in-memory registry as an outage fallback.

## Remove with callers

Retire membership/portfolio databases, generic namespace-owner cutover, full registry replay/restore,
legacy DIRECT/root-catalog authority, receipt-bound currentness and package splits that only move types.
Saved source sets are small current configuration until a real history feature exists. External scholarly
connectors are separate optional work and do not block local repositories.

Preserve #598's active private source-retention work and consume its useful retained revision/head result;
do not create a competing writer. Unsupported pre-release roots may be rejected before writes and left
intact.
