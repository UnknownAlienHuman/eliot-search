# W7 hardening — `search-access`

This packet refines the existing W4 `FUNCTIONS.md` for P13. It does not transfer purge, handle,
continuation, validator or retention ownership.

## Restrictive mutation state machine

```text
VALIDATED
→ CONTROL_COMMITTED
→ LIVE_FENCE_PUBLISHED
→ DEPENDENT_INVALIDATIONS_REQUESTED
→ REQUIRED_INVALIDATIONS_ACKNOWLEDGED
→ ACKNOWLEDGED
```

Recovery states are `PUBLISHING`, `INVALIDATING`, `FAIL_CLOSED` and `BLOCKED`. A durable restrictive
commit is monotonic and never rolled back because a later invalidation/cache/index step failed.

## `classify_security_change`

```text
classify_security_change(old, new) -> SecurityChangeClass
```

Closed classes:

- `RESTRICTIVE`
- `PERMISSIVE`
- `NOOP`
- `INVALID`

Any uncertainty or incomparable policy is restrictive/fail-closed for current admission. Permissive
changes never resurrect purged/tombstoned material and still require normal source/admission/reconcile
processing.

## `install_live_restriction`

```text
install_live_restriction(command, operation, control_port, snapshot_port)
    -> Result<LiveRestrictionReceipt, AccessError>
```

One guarded durable transaction increments security/access or purge generation and records exact scope.
Success is not acknowledged until the immutable live fence is published. Timeout after possible commit
is resolved by operation/generation readback. Cancellation cannot report rollback after commit.

## `build_invalidation_request`

```text
build_invalidation_request(receipt, current_state) -> LifecycleInvalidationSet
```

Names exact binding/principal/grant/source namespace/membership/view/owner generation/route/profile and
purge scope for query admission, active legs, validators, handles, continuations, caches and clients. It
contains no source content or generic vendor filter.

## `accept_invalidation_receipts`

```text
accept_invalidation_receipts(restriction, required_receipts)
    -> SecurityMutationCompletion
```

Missing, stale or mismatched required owner receipt leaves the scope `FAIL_CLOSED`. Security fence remains
live. Ordinary index reclaim or cache expiry never substitutes for handle/continuation/purge receipts.

## Checkpoints

`recheck_live_access` is mandatory:

1. request admission;
2. before every retrieval/IDF/count/facet/trace leg;
3. after every leg completes;
4. before source readback;
5. after source readback;
6. before candidate/result emission;
7. before and after handle expansion readback;
8. before every continuation expansion/replan;
9. before exact match/report emission;
10. before restore/republication admission.

A restrictive generation change between checkpoints discards contaminated work at the causal unit:
whole scoring/IDF leg, candidate, expansion, exact report or restore step. Candidate-only filtering may
not preserve contaminated ordering/counts/traces.

## Active request contamination

```text
classify_active_request_contamination(request_state, old_fence, new_fence)
    -> ActiveRequestDecision
```

Returns `CONTINUE_UNAFFECTED`, `DISCARD_AND_REPLAN`, `DENY`, `CANCEL_AND_GAP` or
`COMPLETE_NON_CONTENT_RECEIPT_ONLY`. No decision may expose inaccessible names/counts/scores or indicate
whether a foreign token/source existed.

## Required tests

- restrictive commit crash at control/snapshot/invalidation/ack boundaries;
- revocation/purge at every checkpoint above;
- whole-leg discard when IDF/candidate population was contaminated;
- handle/continuation expansion cannot race past live fence;
- permissive policy never resurrects purge tombstone;
- missing owner invalidation receipt leaves fail-closed;
- same operation/equal input is idempotent; conflicting reuse rejected;
- no redb write on ordinary grant validation/checkpoint recheck;
- default diagnostics leak no denied source/name/count/path/token.
