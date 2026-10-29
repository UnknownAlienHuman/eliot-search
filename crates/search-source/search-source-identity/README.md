# search-source-identity — reduce to current source matching

A source identity distinguishes one current logical source from another and helps reconcile a rename without
merging unrelated files. It is not a permanent path-history, lineage, workspace or namespace-cutover
operating system.

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md),
[source registry map](../search-source-registry/FUNCTIONS.md) and [source map](FUNCTIONS.md).

## Verified current source

`src/` contains about **149.3 KiB** of Rust:

```text
binding / history                 23.7 KiB
resolution / evidence             39.4 KiB
lineage / workspace               16.8 KiB
NTFS source/directory             18.7 KiB
batch / current / recorded        20.3 KiB
legacy/git digest                  13.5 KiB
redaction / errors / root          15.8 KiB
```

External production callers found:

- native source acquisition uses NTFS volume/file identity, current path-key construction and observation
  validation;
- old `search-source-registry::source_v2` uses `resolve_identity`;
- lineage, path-transition/history, batch and redaction operations were found in this package's tests/docs,
  not in the connected daemon/query product.

## Current model

The source/control owner stores:

```text
SourceId                    // assigned once and persisted in the current source row
RootId
current root-relative locator
optional PhysicalFileKey    // current strong native identity when available
current RevisionRef
availability / current classification
```

`RevisionRef` owns content identity. Equal content does not merge two sources. A path string is a locator,
not proof of physical equality.

For a local filesystem inventory:

1. the same current locator updates the same `SourceId`;
2. an optional exact physical key may match a disappeared old locator to a new locator as a rename;
3. simultaneous aliases/hardlinks may map to one current source only under an explicit finite policy;
4. a conflicting physical key/path observation is explicit, never guessed from content or normalized text;
5. no permanent path interval/history is retained after current state and live views no longer need it.

For NTFS the useful physical key is the exact observed volume identity plus file index and creation time from
the held final handle. Reparse/containment/current permission rules remain with the native safe-reader/root
owner. The key may be stored in a bounded private form; it needs no profile receipt or canonical proof tree.

For Git, source identity is the registered repository/view plus current tree path. The exact commit/tree/blob
OID and retained bytes identify the selected revision/evidence. A remote URL, branch name, repository name or
fork/mirror guess is not lineage authority.

## Remove after current-state cutover

- `PathBindingRecord` interval/history transitions and owner-generation receipts;
- repository/worktree/fork/mirror/proven-copy lineage classification;
- workspace identity and branch/index state identities;
- identity batch framework and currentness records;
- generic observation evidence/fingerprint/canonical recorded forms;
- redacted identity view framework;
- legacy digest compatibility and separate Git stable-identity digest;
- fixtures/goldens/source-shape requirements protecting those APIs.

Keep or move only the small native physical-key/path matching needed by #270/#271. If that logic fits inside
source reconcile, move it there and delete this package.

## Completion

A file is admitted once, assigned one current `SourceId`, survives ordinary edits, follows a proven rename
without duplicate preparation and treats uncertain/path-reused/hardlink observations explicitly. Restart
reads current source rows; it does not replay path history or lineage. No identity package call blocks the
same acquired body from entering immutable retention.
