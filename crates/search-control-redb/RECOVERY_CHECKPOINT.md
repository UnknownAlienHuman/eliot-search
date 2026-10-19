# Historical publication recovery checkpoint

The previous design reconstructed a publication checkpoint from route/visibility records, a retained
intent and the generic operation ledger. That recovery plane is superseded.

The current publication owner keeps one exact `PendingPublication` record pointing to a bounded immutable
operation artifact. Restart reads that record and artifact, checks actual Qdrant/current control state and
continues or resolves the external effect. It does not scan completed control receipts or reconstruct a
coordinator from historical labels.

An `aborted` label alone is not proof that an external effect was compensated; unresolved pending state
remains pending until readback/cleanup says otherwise. After successful visible-view commit, operation-only
cursor/checkpoint data is removed.

See [FUNCTIONS.md](FUNCTIONS.md) and the
[publication recovery map](../search-index-qdrant/search-publication/FUNCTIONS.md). Remove the old
checkpoint/ledger API with its last callers.
