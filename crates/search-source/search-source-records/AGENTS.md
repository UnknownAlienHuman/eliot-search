# Working on search-source-records

This 332 KiB package is a legacy schema/codec bundle scheduled for deletion. Read [README.md](README.md),
the [source registry map](../search-source-registry/FUNCTIONS.md) and
[current admission map](../search-source-admission/FUNCTIONS.md).

Do not add batch/snapshot/operation/currentness/portfolio/namespace/provenance records, compatibility codecs,
canonical identities, receipts or forwarding reexports. Do not migrate old formats into new tables.

When changing a caller:

1. identify the few current `RootRecord`, `SourceRecord` or `RevisionRef` fields it actually needs;
2. move those fields/codecs to the concrete source/control/object owner;
3. point-read or bounded-iterate typed current redb state;
4. delete the old record import, batch/restore adapter and exclusive fixtures in the same cutover.

`PublishedView`, not registry history, owns immutable query membership/evidence. Unsupported old roots fail
before writes and remain intact. Preserve #598's active private work; no competing retained-source writer.

After code, run minimal Rust 1.98 compiler/Clippy for changed source/control/daemon consumers. Old byte
goldens, package maps and replay matrices are removed with the APIs.
