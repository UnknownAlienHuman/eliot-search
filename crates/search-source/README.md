# Sources — one ingestion owner, not seven operating systems

Selected roots become immutable retained objects, current source heads and later published evidence. The
[implementation guide](../../docs/implementation/README.md) defines the connected result.

## Historical baseline overhead

At the original public-main baseline `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`, the following
policy/receipt/provenance/records/history/identity/registry packages contained **more than 1.22 million bytes of
production Rust** before safe-reader, revision-store, reconcile and tests were counted:

| Existing package at that baseline | Gross production source | Selected disposition |
|---|---:|---|
| `search-source-admission` | ~367 KiB | keep one small pre/post-read policy evaluator |
| `search-source-admission-receipt` | ~34 KiB | delete |
| `search-source-provenance` | ~57 KiB | delete |
| `search-source-records` | ~332 KiB | move a few current rows, delete |
| `search-source-history` | ~70 KiB | delete |
| `search-source-identity` | ~149 KiB | keep/move small physical-key/current-source matcher |
| `search-source-registry` | ~214 KiB | replace with typed current redb/source operations |

This is historical gross inventory, not a current recount or promised net deletion. It explains how extensive
infrastructure still failed to connect one acquired file to retention. Current callers must be checked at each
code cut before removing a package.

## Current target

```text
configured root
→ ignore-filtered bounded inventory / watcher hint
→ current admission pre-read decision
→ one stable final-handle or Git-object read
→ current admission post-read classification
→ same owned bytes to one immutable object writer
→ one typed SourceRecord / RevisionRef compare-set
→ one preparation job
→ later PublishedView
```

Owners:

| Responsibility | Owner |
|---|---|
| root lock, containment, native/Git stable read | root/safe-reader composition |
| selected-source policy | one small admission evaluator |
| immutable bytes | one object writer/reader from #598/#330 |
| current roots/sources/heads | typed redb/source methods |
| changed/moved/missing work | one reconcile/inventory queue |
| query evidence | `PublishedView` manifest + immutable reader |

## Identity and state

A path locates a source. `SourceId` is assigned/persisted in current state. An optional exact native physical
key may prove a rename. Content digest identifies a revision, never the source. Git evidence binds the exact
selected tree/path/object ID and retained bytes.

Keep no permanent path history, portfolio/membership history, original admission transcript or operation
archive merely so an old query can reopen data. Old queries hold their immutable `PublishedView` and exact
revision references.

## Package disposition

- Keep/simplify `search-safe-reader`, `search-revision-store` and `search-source-reconcile` around the connected
  path.
- Fold the few useful admission/identity/current-row values into those concrete owners.
- Delete receipt, provenance, records, history, restore, batch, cutover, portfolio and lineage packages after
  direct callers move.
- If the remaining registry/identity code is only typed redb methods and a small matcher, delete those crate
  boundaries too.

Unsupported old roots are refused before writes and left intact. There is no migration or legacy-reader
prerequisite. Preserve #598's active private source work and consume only its accepted handoff.
