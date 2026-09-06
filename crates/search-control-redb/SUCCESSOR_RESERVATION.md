# Normal committed publication succession

`PublicationSuccessor` and `reserve_next_publication` permit the normal next-epoch
path after a verified `CONTROL_COMMITTED` predecessor. The ordinary `begin`
continues to reject replacement of an existing intent.

The request binds the full predecessor intent, control generation, complete
route/visibility/guard state and new prepared intent. Its epoch must be exactly
the predecessor's epoch plus one; overflow, a reused intent identity, stale guards
and all non-committed predecessors are rejected. Aborted, blocked, compensating,
invalidation-only and reclaimable predecessors are deliberately not admitted by
this normal-path API. They need their own complete recovery/finalization protocol.

Before new forward progress, one coherent read validates the current typed state,
latest publication receipt and its actual operation. All retained publication
receipts are checked for reuse of the proposed intent identity. The owning
snapshot publisher must be disk-bound to this journal, unsuspended and exactly
match the current durable generation and records. An unbound reference-model
snapshot, unpublished commit, stale snapshot or missing receipt is insufficient.
These checks do not grant external root ownership or verify Qdrant effects.

The existing conditional transaction atomically compares the entire prior intent
and visibility record, plus global generation, then changes only the intent.
VisibleEpoch, all manifest/shadow records and every prior publication receipt stay
unchanged. The previous committed intent remains represented by its durable
publication receipt. No new physical table, key family, codec, dependency or
schema version is added: the existing schema-3 validation already admits an
unresolved next-epoch intent alongside the previous visible receipt. Schemas 1
and 2 cannot use the new API; no existing database is migrated or relabelled.

Exact replay is not new forward progress. The common engine checks the full
conditional-request fingerprint and post-state; it does not require the old
published snapshot again or overwrite newer progress. `recover_publication_successor`
uses the exact original request, never a command rebuilt from current values.
Cancellation before dispatch makes no changes. Interruption after possible write
retains the existing unknown-outcome fence until exact recovery.

A reservation is not publication acknowledgement. Existing in-flight readers may
hold the previous committed snapshot subject to live restrictive barriers. The
new unresolved intent cannot be published as a completed control snapshot. After
its own visibility commit, its snapshot must be published before another fresh
reservation. Daemon composition must serialize these operations and supply the
sole owning publisher; a clone or an old Arc is not an authority lease.

Fifteen tests plus one ignored subprocess helper cover two complete publications
and a third reservation, unchanged visibility/manifests, pending/aborted rejection,
ID reuse, overflow, snapshot provenance, competing requests, exact replay/recovery,
owner handoff, foreign/stale snapshots, same-generation corruption, cancellation/deadline and bounded child
exit before/after the real commit boundary. Fixed IDs/manifests are test fixtures;
these tests are not live Qdrant or power-loss qualification.

Verification in the authoring environment: Rust compile/tests/fmt/Clippy NOT_RUN;
Cargo is absent. Patch/context/source checks are not a passing Rust build. No gate,
T09 acceptance, H5-table qualification or primary daemon cutover is claimed.

```sh
cargo +1.98.0 test --locked -p search-control-redb --lib
cargo +1.98.0 check --workspace --all-targets --all-features --locked
```

The new module and tests extend the existing private transaction boundary. Based on
the recorded 9,212-line baseline, this increment adds 542 raw Rust lines (including
comments, blank lines and tests), remaining below the 10,000 hard ceiling. The prior
split-review trigger remains open; this is not independent review or a gate receipt.
