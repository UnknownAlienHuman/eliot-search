# search-source-registry — source map

The [implementation guide](../../../docs/implementation/README.md) owns the connected source path. This
file distinguishes current code from product operations that actually have callers.

## Implemented API and caller status

| Area | Current behavior / caller reality |
|---|---|
| Source-v2 preparation/commit/recovery | Exported through `api.rs` and used by daemon registry-control/native-source composition. It persists admission/original-input metadata. The acquired body is not retained and output still reports no retained revision. |
| `register_root`, `admit_source`, `bind_membership`, transitions | Generic in-memory `SourceRegistry` mutation plus a vendor-neutral control port and receipt. Searches found root/membership calls predominantly in package or registry-control fixtures rather than the normal source command. |
| `publish_reference_portfolio` | Implemented and exercised by package tests. No serving/query producer was found. |
| `resolve_source_view` / workspace view / verify view | Pure snapshot/view functions and a source port adapter. No connected product query uses this registry view as the published Qdrant/evidence view. |
| Namespace cutover | Pure cutover preparation/fence/activation/recovery over registry state. No normal source writer consumes it. |
| Batch/restore | Replays complete legacy registry histories and operation receipts. This is not the baseline current-format source open path. |
| Source revision registration | Reexported from `search-source-history`; searches found test callers, not the native acquired-body path. |
| `search-source-records` / `search-source-history` | Relocated legacy codecs, currentness, revision and restore code that the registry reexports. They do not own source bodies or a separate database. |

## Connected source operation

Implement one coherent operation around the exact bytes already acquired by the daemon:

```text
pre-read admission
→ stable final-handle read
→ post-read classification
→ immutable revision-object write/readback
→ short redb transaction:
     upsert SourceRecord
     set current RevisionRef
     record availability/locator
→ enqueue preparation/publication work
```

The object writer runs before the source-head transaction. A failure before commit leaves an unreferenced
object for bounded GC. A known transaction abort is a known failure. An actually unresolved commit is
resolved by reading the current source/head record; do not rescan the pathname or append another generic
operation receipt.

## Target typed operations

Exact names may change with current code, but the owner needs only:

- `register_root` / `update_root` / `unbind_root` for selected local roots;
- `get_source` / `list_sources` / consistent current snapshot;
- `commit_source_head(source, expected_head, revision_ref, locator, availability)`;
- `retire_source_head(source, expected_head, reason)` without deleting retained objects;
- `resolve_selected_sources(requested_ids, current_snapshot)` with no widening;
- source/reference enumeration for publication cleanup.

Use existing redb transactions directly. Package-level in-memory journals and receipt collections are not
the persistent implementation.

## Published view versus registry snapshot

A registry snapshot is current control input. `PublishedView` is the immutable query object produced after
preparation and Qdrant publication. Do not rebuild permanent corpus/portfolio/source-view history in the
registry. An old query stays coherent because it holds the old published-view guard and manifest, not
because every prior registry alias remains forever queryable.

## Delete after cutover

- reference portfolio/history and membership policy machinery with no real user-facing consumer;
- namespace export/cutover platforms and universal migration replay;
- legacy DIRECT journal/root-catalog codecs from the new application path;
- process-local currentness/receipt authority;
- `search-source-records` / `search-source-history` forwarding splits once useful current types move to
  their actual owner;
- generic control-port operations and permanent operation archives replaced by typed redb changes.

Keep read-only legacy code only while an explicitly selected old-format diagnostic needs it. It cannot
write current state or block a clean current-format implementation.

## Behavior to verify

- the exact acquired body is the body referenced by the committed revision;
- same source/same revision replay and same expected head/different revision conflict;
- rename without content change updates location without duplicating preparation;
- missing/partial inventory cannot mass-retire unseen sources;
- restart reconstructs current roots/sources/heads from redb;
- publication receives one consistent source snapshot;
- old published-view readers keep exact evidence while new source heads advance;
- no package test-only portfolio/cutover/restore path remains reachable from normal ingestion/query.
