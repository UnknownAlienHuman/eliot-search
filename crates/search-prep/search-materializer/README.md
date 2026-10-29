# search-materializer

Convert one retained object into text, source-coordinate maps and useful document/code structure.
Materialization owns transformations and their assurance; it does not own another object store or
publication lifecycle.

Read the [implementation guide](../../../docs/implementation/README.md) and
[FUNCTIONS.md](FUNCTIONS.md).

## One preparation pass

```text
committed retained revision
→ one verified object read
→ one MaterializationProduct
→ structure-first UnitSet
→ one PreparedArtifact
→ shared immutable object writer
→ short current-preparation reference in redb
```

Markdown/JATS/plain/code unitization consumes the private immutable product directly. Rebuild/reparse only
when loading an independently persisted or external artifact, not between adjacent trusted functions.

## Current duplication

The text/product pipeline is useful. The legacy artifact platform is not: its contextual publication
kernel is about 40 KiB before tests and mirrors revision-store's stage/write/sync/publish/readback code.
It can leave the staging name after successful publication. Daemon preparation then performs additional
complete reads/copies of plaintext artifacts.

Delete that platform after prepared artifacts use the shared current-format object writer. Materializer
continues to define prepared payload/maps/structure, but filesystem publication and GC have one owner.

## Allocation discipline

- Move the owned UTF-8 buffer into `String` when it is already valid and unchanged.
- Preserve exact text without allocating a second identical normalization buffer.
- During UTF-16 decode, collect coordinates/line boundaries in one pass instead of retaining multiple
  scalar-sized temporary arrays.
- Keep parser offset iterators and borrowed XML events from the existing donors.
- Bound actual parsed nodes/events/maps/units and output bytes before retaining them.

Unsupported encoding, lossy mapping, cancellation, deadline or truncation yields a typed gap/error. It
never produces zero digests, `invalid` profile strings or a complete-looking checkpoint.
