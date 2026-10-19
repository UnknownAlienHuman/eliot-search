# search-source-registry — source map and replacement

The [implementation guide](../../../docs/implementation/README.md) owns the connected source path. This
file distinguishes current source from the few current-state operations the product actually needs.

## Current normal command sequence

`DirectRegistryOwner::prepare_output` and `prepare_source_output` execute approximately:

```text
bind `RegistryJournal` through `bind_original_v4`
→ restore complete `SourceRegistry`
→ load historical operation by operation ID
→ reconstruct original v4 policy/admission inputs
→ requalify root/source records
→ prepare and commit a generic registry batch
→ read all changed targets back
→ drop the adapter
→ rebuild/publish the complete control snapshot
```

For a new source, `native_source_command` additionally performs the stable read and Source-v2 admission,
but the acquired body is dropped and output still reports no retained revision.

The adapter supporting this path stores pending generic requests, operation envelopes and original-input
recovery. Its supporting non-test files under `bins/eliot-searchd/src/registry_control/` already exceed
about 63 KiB before the large main adapter. This is not a small source-head transaction.

## Package API and caller reality

| Area | Current behavior / caller reality |
|---|---|
| Source-v2 preparation/commit/recovery | Used by daemon source admission. Persists admission/original-input metadata; does not retain the acquired body or commit a usable `RevisionRef`. |
| Generic root/source/membership operations | Mutate an in-memory `SourceRegistry` through `RegistryControlPort`, operation IDs, mutation digests and receipts. Many callers are fixtures or the restore adapter. |
| `registry_snapshot` | Clones roots, sources, memberships, portfolios, portfolio history, active generations, namespace ownership and namespace state into another aggregate. View helpers call it again. |
| `publish_reference_portfolio` | Implemented and exercised by tests; no connected local query producer was found. |
| source/workspace view resolution | Pure snapshot/view functions over the cloned aggregate; not the immutable Qdrant/evidence `PublishedView`. |
| namespace cutover | General owner transition/fence/activation/recovery over registry state; no normal source writer requires it. |
| batch/restore | Replays complete registry histories and operation receipts. The normal daemon currently depends on it, but the current-format target must not. |
| source revision registration | Reexported from `search-source-history`; test callers were found rather than the native acquired-body path. |
| forwarding crates | `search-source-records` and `search-source-history` relocate legacy codecs, currentness, revision and restore code; they do not own source bytes or a database. |

## Connected source operation

Implement one coherent operation around the exact bytes already acquired by the daemon:

```text
pre-read admission
→ stable final-handle read
→ post-read classification
→ #330 immutable object write/readback
→ short typed redb transaction:
     upsert SourceRecord
     compare/set current RevisionRef
     update locator/availability
→ enqueue #331 preparation/publication work
```

The object writer runs before the source-head transaction. A failure before commit leaves an unreferenced
object for bounded GC. A known transaction abort is a known failure. A genuinely unresolved commit is
resolved by point-reading the current source/head row; do not rescan the pathname or inspect an operation
archive.

## Target typed state and operations

Suggested table/key ownership may follow the accepted current schema, but the behavior is:

```text
ROOTS[root_id]                  -> RootRecord
SOURCES[source_id]              -> SourceRecord
SOURCE_BY_LOCATOR[root, locator] -> source_id        // only when current lookup needs it
SAVED_SELECTIONS[name]          -> ordered source IDs // optional current config
```

Operations:

- `register_root` / `update_root` / `unbind_root`;
- `get_root`, `get_source`, bounded `list_roots` / `list_sources`;
- `commit_source_head(source, expected_head, revision_ref, locator, availability)`;
- `retire_source_head(source, expected_head, reason)` without deleting retained objects;
- one consistent read transaction that enumerates current source heads for reconcile/publication;
- `resolve_selected_sources(requested_ids, PublishedView)` with no widening.

Use exact point reads for command/update paths. Use bounded table iteration only when an actual inventory or
publication snapshot needs all current rows. Do not reconstruct the result by replaying prior operations.

## Published view versus current registry

A current source snapshot is mutable control input. `PublishedView` is the immutable query object produced
after preparation and Qdrant publication. Do not rebuild permanent corpus/portfolio/source-view history in
the registry. An old query stays coherent because it holds the old published-view guard and manifest, not
because every previous registry alias or operation remains queryable.

## Delete after cutover

- `RegistryJournal`, original-input layouts/resolvers and v4 restore/readback;
- registry batch/operation codecs used only for current-command replay;
- complete in-memory `SourceRegistry` restore before each command;
- aggregate `registry_snapshot` copies on the connected query path;
- reference portfolio/history and membership policy machinery with no current consumer;
- namespace export/cutover and universal migration replay;
- legacy DIRECT journal/root-catalog codecs from the new application path;
- process-local currentness/receipt authority;
- forwarding package splits after useful current types move to the typed control/source owner.

## Behavior to verify

- exact acquired body is the body referenced by the committed revision;
- same source/same revision replay and same expected head/different revision conflict;
- rename without content change updates location without duplicate preparation;
- missing/partial inventory cannot mass-retire unseen sources;
- restart point-reads current roots/sources/heads directly from redb without operation replay;
- publication reads one consistent current source snapshot;
- old published-view readers keep exact evidence while new source heads advance;
- no portfolio/cutover/restore path remains reachable from normal ingestion/query.
