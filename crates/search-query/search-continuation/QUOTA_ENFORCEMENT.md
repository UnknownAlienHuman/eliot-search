# Continuation record and issuance quotas

Date: 2026-09-22. Tracking: PR #118 / T21.
Base: `a71037cee7f229f2adbec12b594acfb3c3c9eb7f`.
Scope: the existing canonical `search-continuation` owner, not daemon cutover.

## Retained records are not active records

`max_records` counts every retained record, including terminal records awaiting
external cleanup and compaction. Previously `apply_live_limits` compared only
active survivors with the new cap, expired some of them, and returned success.
Expiration does not remove record or token-index entries: the supposedly
successful configuration could therefore already violate its own record cap.

The method now rejects a cap below the total retained count before any record,
limit, token index or cleanup reference changes. The existing
`ResourceExhausted` error is used. It never deletes denial records or assumes
that their external cleanup has completed just to make a configuration fit.
The obsolete active-only count-reduction loop is removed. Per-binding, window,
issued-set and TTL restriction selection retains its existing ordering and
all-before-write batch preparation.

To reduce the retained cap, integration must keep its admission/configuration
barrier closed, explicitly select invalidations or expiry where required,
execute the returned external cleanup effects, compact terminal records in
bounded calls, and retry the limit change. The pure owner does not choose an
external eviction policy or acknowledge cleanup. At the exact cap the update
is valid, but admission remains full until a record slot is actually removed.

## Do not return an unfulfillable expansion

Previously `resume` selected up to the request size without considering the
remaining issued-fingerprint quota. It could return a page and pin-renewal
request that the later emission checkpoint necessarily rejected, or ask a
durable executor for results when no more fingerprints could be recorded.

Resume now checks the remaining per-record issued quota after the unchanged
credential and live-fence checks. Ephemeral selection is bounded by the request
and remaining quota. A full quota with unissued candidates returns
`ResourceExhausted`, not `Exhausted`; a genuinely fully issued window retains
its explicit exhausted/completion path. No result, pin-renewal request or replan
is returned on quota refusal, and the existing window/history remains intact.

Durable pending permits retain the same effective bound. The new read-only
`ContinuationPermit::max_emission_items()` exposes that bound to the executor;
for an already-bound selection it returns its cardinality, and for exhaustion
it returns zero. An increased limit cannot widen an existing permit. Current
quotas and live authority still need revalidation before delivery. Existing
post-delivery accounting, partial selected acknowledgement and cleanup remain
unchanged. Exhausted quota does not block explicit scoped invalidation/expiry.

This adds one package-owned accessor, not a shared field or new state owner.
Existing signatures, reason codes, defaults, dependencies, persisted formats
and workflows remain unchanged. Resource refusal is not permission to clear
issued history, switch corpus, remint or disclose under a stale live barrier.

## Verification boundary

`src/quota_tests.rs` adds 15 synthetic Rust tests, covering every active/terminal
placement in a three-record store for both durabilities, expiry, exact capacity,
failed combined changes, compaction/retry/admission, per-binding order, invalid
limits, residual issuance, true exhaustion, durable binding, immutable permit
bounds, per-record isolation, security-error precedence and explicit cleanup.
The small-window matrix covers 48 request/issued/cap combinations. All previous
test files remain unchanged; fixtures are not native or external-effect proof.

The reconstructed baseline matched Git blob
`e872096a1ddfc7a5a4e5d117af69f3985d935aca` (49,439 bytes).
Scoped diff, source-preservation and public-signature checks were performed.

Required exact-head checks:

```text
cargo +1.98.0 test --locked -p search-continuation --lib
cargo +1.98.0 check --locked -p search-continuation --all-targets
cargo +1.98.0 fmt -p search-continuation -- --check
cargo +1.98.0 clippy --locked -p search-continuation --all-targets -- -D warnings
```

Rust compile/tests/rustfmt/Clippy: **NOT_RUN**. The actual targeted test attempt
exited 127 (`cargo: command not found`). The local tree is scope-only; direct
GitHub/Rust downloads failed. No Actions run, measured performance, independent
review, accepted handoff, external cleanup or T21/product acceptance is claimed.
