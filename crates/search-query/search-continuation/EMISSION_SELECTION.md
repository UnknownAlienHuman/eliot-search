# Continuation emission selection

Date: 2026-09-21. Tracking: PR #118 / T21.
Base: `05a0bc0062454ddd4df865c0c70b573da0c1da35`.
Scope: the existing canonical `search-continuation` owner, not daemon cutover.

## Defect

`resume(max_items)` selected a bounded page, but its permit retained only the
record revision, binding and plan/result fences. `commit_emission` checked
membership against the entire retained candidate window. A permit for one
candidate could acknowledge a different candidate or a larger set outside that
page. Durable replans could acknowledge arbitrary new fingerprints without a
bound replan result or the original request's item limit.

## Corrected flow

- Ephemeral permits retain exactly the fingerprints selected by that resume.
  Acknowledgement accepts a nonempty subset of that set, preserving partial
  successful-delivery accounting. Other retained or foreign candidates fail.
- Exhausted observations cannot acknowledge candidates. Explicit `complete`
  remains available for completion/abandonment and returns cleanup, not proof
  of corpus completeness or successful delivery.
- Durable resume returns a pending permit with the original `max_items` bound.
  Call `bind_durable_emission` after the accepted planner/executor and source
  validator produce the bounded result. It rechecks the original record and
  live state, then freezes unique, unissued fingerprints within both original
  and current quotas. Binding changes no store, issued set, pin or deadline.
  A pending permit cannot acknowledge anything; a bound permit cannot be
  rebound. Competing bindings share one revision and cannot both commit.
- `revalidate_emission` is a read-only pre-delivery checkpoint for both
  durabilities. It checks the original selection and record incarnation, live
  authority, plan/result fences, pin/job state, original expiry, current quotas
  and the next revision. Cancellation before acknowledgement marks nothing
  issued. The same preparation rules back `commit_emission`, preserving its
  all-before-write returned-error atomicity.
- Permits also bind the record token digest, creation and expiry timestamps.
  Matching continuation ID and revision alone cannot reuse a permit for a
  record with a different token incarnation or lifetime.

`emission.rs` owns selection/preparation/accounting logic inside the same crate.
It introduces no second store. Only bounded fingerprint sets are retained in
permits; source windows are not copied into them. The old full-window membership
set allocation at each acknowledgement is removed. No latency/RSS claim is made.

## API and integration boundary

Existing public method signatures and shared contract records are unchanged.
Two methods are added: `bind_durable_emission` and `revalidate_emission`.
**Durable call sequencing intentionally changes:** acknowledging the pending
`ResumePlan::DurableReplan` permit directly now returns `StalePermit`.

```text
resume
  ephemeral: selected candidates + bound permit
  durable:   pending permit -> execute/validate -> bind_durable_emission
-> fresh live observations -> revalidate_emission
-> deliver under the existing live security/output barrier
-> commit_emission with exactly the successfully delivered subset
```

Caller-supplied durable fingerprints do not prove execution or source validity.
The integration owner must supply verified results under the original fence;
this package neither runs the backend nor authorizes new source evidence.
Revalidation and output are not atomic by themselves: integration must hold its
security/output barrier, handle cancellation and output failure, and apply
cleanup effects. Post-delivery accounting has no clock/live arguments and must
not be used as permission to disclose. No receipt proves external pin cleanup,
checkpoint persistence, native execution, or socket delivery.

The daemon still uses its legacy catalog. This increment does not claim T20/T21
acceptance, canonical cutover, durable backend integration or a live test gate.
No dependency, shared schema, error code, persisted format or workflow changes.

## Regression source and verification

`src/emission_tests.rs` adds 20 deterministic synthetic tests, including every
nonempty subset of a three-item window at three request sizes. They cover
partial acknowledgements, stale clones, invalid selections, pending/bound
replans, original/current/issued quotas, both durabilities, every live fence,
expiry, nonmutating checkpoints, revision exhaustion, invalidation and reused
ID/revision with changed token or lifetime.

All 16 prior atomicity test bodies remain byte-identical. Their fixture helper
now binds synthetic durable results through the new API; helper visibility is
shared with the new tests rather than duplicating the fixture implementation.

Baseline source blobs were reconstructed exactly: `ea577e6` and `a815f10`.
Scoped diff, test-body preservation and source-routing checks were performed.
The local tree is scope-only; it is not a full workspace clone.

Required exact-head execution:

```text
cargo +1.98.0 test --locked -p search-continuation --lib
cargo +1.98.0 check --locked -p search-continuation --all-targets
cargo +1.98.0 fmt -p search-continuation -- --check
cargo +1.98.0 clippy --locked -p search-continuation --all-targets -- -D warnings
```

Rust compilation/tests/rustfmt/Clippy: **NOT_RUN**. The actual library-test
attempt exited 127 (`cargo: command not found`). No Actions run was started.
Added test functions are not passing execution evidence or independent review.
