# Working after lifecycle-platform removal

The generic retention/purge/sweep/restore implementation is gone. This package intentionally contains no public
API; do not add a compatibility facade, placeholder state machine or new code here.

Read [FUNCTIONS.md](FUNCTIONS.md) for the selected local-object, archive and import behavior. Implement those
operations in the concrete store/control/publication owners. Read
[publication](../../search-index-qdrant/search-publication/FUNCTIONS.md) for live Qdrant cleanup.

The next Cargo cut removes this package from the workspace, daemon optional dependencies, `wave7-lifecycle` and
`Cargo.lock` together. Until then, the empty crate exists only to keep the current manifest/lock graph coherent.

Protect original files, archives and sole retained copies. Code first for a selected owner, then minimal pinned
check/Clippy for changed production owners and real consumers. Runtime/platform/restore claims require execution.