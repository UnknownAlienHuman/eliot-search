# Working on search-source-history

This package is a 70 KiB forwarding restore/history layer scheduled for deletion. Read [README.md](README.md),
[search-source-records](../search-source-records/README.md) and the
[source registry map](../search-source-registry/FUNCTIONS.md).

Do not add revision history, namespace cutover, registry replay, receipts, compatibility codecs or new
callers. Do not preserve the package merely to reexport moved value types.

After #598's accepted handoff and #270/#271 current-state cutover:

- move the small current `RevisionRef` fields/validation to the object/source owner;
- use typed redb current source-head reads/writes;
- keep old revisions through current/pending/cleanup/live-view references;
- delete restore/cutover/history code, tests and dependencies.

Unsupported old roots remain intact. No automatic migration or universal history replay.

Run minimal Rust 1.98 compiler/Clippy for actual source/object/control/daemon consumers after production
changes. Registry replay fixtures and old byte compatibility are removed with their APIs.
