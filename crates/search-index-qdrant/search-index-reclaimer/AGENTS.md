# Working on the old index reclaimer

This package is a removal target. Publication owns cleanup.

The baseline does not physically delete individual points. Replaced points stay closed by their immutable
`valid_until` boundary until a rebuild/compaction switches to a fresh never-reused generation.

Move only whole-generation deletion/readback needed by publication:

```text
retired generation
+ no live PublishedView/QueryWindow guard
→ delete exact collection
→ verify absence
→ remove RetiredCleanup row
```

Local immutable-object collection is separate stopped-root maintenance. Do not preserve point manifests,
watermarks, possible-writer receipts, operation journals or checkpoint machinery. Delete this package and its
fixtures after the generation cleanup caller moves. Run minimal compiler/Clippy for publication/bridge/control
consumers after code.
