# Restrictive security mutation coordinator

T20 / #117; base `f9c8651112cafd762b128442d358cc3978a0fbd3`, 2026-09-22.
Authority: `FUNCTIONS.md` — Live security operations; `W7_HARDENING.md` —
Restrictive mutation state machine, Checkpoints, Required tests.

`SecurityMutationBarrier` in `src/barrier.rs` owns one domain transition:

```text
block checkpoints -> durable commit/readback -> publish immutable restriction
-> verify every required dependent acknowledgement -> return completion
```

Only increasing, membership-restrictive changes are accepted. Existing deny/purge
members cannot disappear. The command binds the exact expected snapshot, policy
revision, domain, operation ID and configured dependent-owner set. The requester
cannot omit an owner. Snapshots use shared set bounds; dependents are capped at 64.

All external errors, cancellation reported by an adapter, and mismatched receipts
retain the pending operation and deny checkpoints. An adapter unwind also leaves
pending work blocking access. Retry resolves the SAME operation through readback;
it never blindly commits again. Proven absence may retry that identity only while
the exact expected control head remains current. A known committed receipt cannot
change/disappear. Publication and invalidation are replayed idempotently; no rollback
of durable restrictions is attempted. Missing, duplicate, foreign or stale dependent
receipts prevent completion. Returned receipts are sorted by owner.

`from_pending_restriction` restores unresolved control/journal input closed.
`from_recovered_snapshot` requires integration-proved startup recovery and published
state; it is not a recovery verifier. The coordinator retains one pending operation
and one completed retry receipt, not a history database. An identical last-completed
retry returns its receipt; older stale replay is refused, not reapplied. Historical
receipt lookup belongs to durable control. Completion is not an access permit.

Integration must provide concrete `SecurityMutationEffects` backed by the designated
control, snapshot and dependent owners, with exact CAS/idempotency, bounded I/O and
qualified cancellation. Their success must come from executed effects, not echoing
inputs. All serving/mutation operations must share this domain owner/lock and use
`with_live_checkpoint`; cached snapshots do not confer authority. Grant validation,
permissive updates and source/policy compilation retain their existing owners.

The concrete effect adapters, durable codec and daemon wiring are NOT implemented
by this increment. No persisted format, dependency, workflow, shared contract or
existing access-compiler behavior changes. No native or T20/T21 acceptance claimed.

Eight public-API tests cover phase faults, readback recovery, receipt substitution,
partial invalidation, unwind/restoration, monotonicity, bounds and replay. Existing
tests are unchanged. Baseline Git hash, source preservation, delimiter and diff
checks passed; these are not Rust execution. `cargo +1.98.0 check --locked -p
search-access --all-targets` exited 127: Cargo is absent. Compilation, tests, rustfmt
and Clippy are NOT_RUN. Required after checkout: that check, package `test
--all-targets`, `fmt -- --check`, and `clippy --all-targets -- -D warnings` (locked).
