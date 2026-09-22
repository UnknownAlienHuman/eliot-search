# Actual continuation lifetime limits

Date: 2026-09-21. Tracking: PR #118 / T21.
Base: `d7dab412efb5a26cc8dfc9dd52e2678c7f1fd7f3`.
Scope: the existing canonical `search-continuation` owner; no daemon cutover.

## Defects

Creation checked the supplied `ttl_millis` against the configured maximum but
only compared `created_at < expires_at`. A caller could supply a one-second TTL
with an expiration a minute or a year later. Restrictive live-limit application
checked candidate/count quotas but never the existing records' lifetimes.
Expansion also accepted a supplied current time earlier than record creation.

## Correction

- Creation requires a positive actual timestamp interval no longer than the
  declared TTL, which must itself fit the configured maximum. A shorter actual
  interval remains valid; creation does not rewrite the requested expiration.
- The private `lifetime` module projects the already-validated canonical UTC
  timestamps to integer microseconds. It follows the shared contract's Gregorian
  calendar and years 0001..9999, without changing shared validation, timestamp
  spelling, timezone or leap-second policy. There is no new date dependency.
- Comparison does not round down to milliseconds. One microsecond above the
  ceiling fails. The TTL is widened before multiplication, including u64::MAX.
- Live-limit application expires incompatible active records of either existing
  durability. It does not silently shorten a token-bound timestamp, renew a
  lifetime or revive an expired record. Overlapping quota reasons select each
  record once; exact cleanup effects are returned in the existing receipt.
- Revision/batch validation and complete receipt preparation precede mutation,
  using the existing atomic terminal-batch path. A failed update changes neither
  records nor effective limits. Integration must keep its configuration/security
  barrier closed when the update fails.
- Resume, durable-result binding and pre-delivery revalidation use the same
  half-open interval: `created_at <= now < expires_at`. A current-time observation
  outside it returns `SnapshotExpired` without changing the record or issued set.

Public signatures, shared records, reason codes, token formats, default limits,
selection binding and post-delivery accounting are unchanged. Old permits for
records expired by the new TTL fail the existing status/revision checks.

This is lifetime-bound enforcement, not qualification of the caller's clock,
monotonic-clock rollback detection, cancellation/output serialization, external
pin cleanup, durable storage or canonical daemon integration. `resolve` has no
clock argument and remains lookup, not permission to disclose. Cleanup effects
are requests to their external owners, not proof of completed cleanup. No workflow,
shared registry, launch state, dependency or persisted-format change is included.

## Regression source and verification

`src/lifetime_tests.rs` adds 16 tests covering both durabilities, false declared
TTLs, exact/short/sub-millisecond intervals, equal/reversed dates, calendar and
century boundaries, the full timestamp range, u64 limits, all dates of a 400-year
Gregorian cycle, selective cleanup, first/middle/last revision faults, batch
refusal, idempotence, stale permits, all pre-delivery time checkpoints, new
admission after reconfiguration and combined quota reasons. Prior test files
are unchanged. Fixtures do not qualify entropy, external clocks or live backends.

The reconstructed baseline matches Git blob `4694e17640513020b5b3feeb641dcc82532c745f`.
Scoped diff and public-signature preservation were checked. An independent local
Python reference checked the ordinal arithmetic against `datetime` for all
3,652,059 valid dates in years 0001..9999 and checked integer-range boundaries.
That reference check does not execute the Rust implementation.

Required exact-head checks:

```text
cargo +1.98.0 test --locked -p search-continuation --lib
cargo +1.98.0 check --locked -p search-continuation --all-targets
cargo +1.98.0 fmt -p search-continuation -- --check
cargo +1.98.0 clippy --locked -p search-continuation --all-targets -- -D warnings
```

Rust compilation/tests/rustfmt/Clippy: **NOT_RUN**. The actual library-test
attempt exited 127 (`cargo: command not found`). The local tree is scope-only,
not a workspace clone. No Actions run, independent review, accepted handoff or
T21/product acceptance is claimed.
