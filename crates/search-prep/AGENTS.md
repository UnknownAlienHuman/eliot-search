# Working on preparation

Read the [architecture](../../docs/architecture/README.md) and
[implementation](../../docs/implementation/README.md). Preparation turns a retained revision into
searchable units while preserving the way back to its bytes.

Decode/parse once and pass the owned validated result to unit construction. An in-process immutable
product does not need a second full parse to prove itself. Verify loaded artifacts at their actual
storage boundary; do not replace that check with a supplied digest label.

Keep a profile for transformations and usable source coordinate/loss maps where bytes change.
A tolerant syntax parse does not establish compiler facts. Preserve those semantics while removing
repeated text copies, document inventories, canonical records and duplicate immutable-file engines.

Change materializer, unitizer and their concrete consumers together. Their current package separation
is optional; no new generic preparation pipeline or crate is required by a format name.
