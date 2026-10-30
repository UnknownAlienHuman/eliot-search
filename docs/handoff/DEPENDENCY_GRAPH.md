# Data and lifetime graph

Navigation, not another algorithm or mandatory crate layout. [Implementation](../implementation/README.md#owners)
owns the stage/owner map; [code map](../implementation/CODE_MAP.md) locates actual files.

```text
settings → root guard → object + typed control owners
stable acquired bytes → ObjectRef → guarded current source row
→ preparation/outcome → fixed Qdrant generation build
→ matching evidence/current commit → PublishedView
→ selected Search/Exact → verified excerpt → local client
```

## Context

Background/root/object work uses execution-only deadlines/counters. Search adds pinned view/selection/scoring;
Exact needs no scoring. Continue/Expand keep the window's view. See [shared meanings](../implementation/README.md#minimal-cross-owner-values).

## Platform finalization

Root/object writers reuse the [standard lock and no-clobber helper](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md#shared-no-clobber-publication).
No crypto profile or bespoke Windows security finalizer joins this path.

## Publication and lifetime

Published views retain exact collections/evidence. [Publication](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md#view-lifetime-and-whole-generation-cleanup)
owns whole-generation cleanup after readers and unresolved writers are gone. Local-object GC is stopped-root work.

## Lock order

Root guard outlives stores. External I/O happens outside short redb writers/view locks. Commit matching metadata,
open its snapshot, then install the view. Prepare page state before output and apply its infallible local update
only after complete transport output. See [the canonical order](../implementation/README.md#lifetime-and-lock-order).

## Optional leaves

Git, formats, models, watcher and adapters join actual consumers only after the selected baseline. Encryption,
restricted users and remote serving are outside [local scope](../architecture/LOCAL_SCOPE.md), not future prerequisites.
