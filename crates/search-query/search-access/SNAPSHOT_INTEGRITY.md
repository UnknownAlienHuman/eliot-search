# Access snapshot integrity

Tracking: T20 / PR #117, prerequisite of T21 / PR #118. Date: 2026-09-22.
Base: `207c59ccfc22d19dc9eae71cdbce1d58e510f202`.

## Membership identity

The scope map key was used for grant intersection and admission's live fence,
while its embedded `MembershipAccessBinding::membership_id` was used for
retrieval/IDF plans. A malformed server snapshot such as key A -> binding B
could therefore admit A but compile B, even when B was outside the grant or
present in the live deny/purge set. With grouping, leg membership keys and
predicate membership IDs could disagree. This is a demonstrated source-level
consistency defect; remote control of the server snapshot is not established.

`intersect_scope` and `compile_safe_legs` now share one key/identity validator.
Both refuse a selected mismatch with the existing `ACCESS_SNAPSHOT_STALE` code.
The latter also rejects empty scopes, including an empty grouping proof; its
publicly constructible input cannot bypass intersection validation. No rekeying,
payload repair or post-filter fallback is performed. Existing inactive, foreign,
unknown and zero-leg-budget failures keep their prior priority. Only selected
bindings are inspected; an unrelated registry entry cannot widen the request.

## Live snapshot continuity

The contamination classifiers could report `Clean`/`ContinueUnaffected` after
receiving an older live generation, or different snapshot contents under the
same generation. They now discard all supplied legs / return `CancelAndGap`
for such inconsistent transitions, including content-free completion. A leg
newer than the current live snapshot is individually discarded. Predicate
retention refuses any plan newer than the observed live generation with
`ACCESS_SECURITY_FENCE_STALE`, returning no partial list.

Valid increasing generations retain existing newly-denied/newly-purged whole-leg
selection. Unrelated and permissible newer changes are not globally rejected;
no deny-set superset rule is introduced. Immediate fail-closed/purge/revocation
errors retain priority in predicate checks. Comparisons need no new allocation,
counter increment, digest algorithm, clock or dependency.

## Scope and verification

Authority: `FUNCTIONS.md` sections "Grant and scope operations" and "Live
security operations"; `W7_HARDENING.md` sections "Checkpoints" and "Active
request contamination". Public signatures, shared shapes, existing reason codes,
predicate digest bytes for valid inputs and all existing tests are unchanged.

Two public-API integration test files add 15 synthetic tests: seven scope tests
(including 378 key/payload/subset/grouping combinations at both boundaries) and
eight live-snapshot tests. They cover malformed and valid DIRECT/Lexical
admission, scope isolation, refusal ordering, empty/stale proofs, rollback,
equal-generation conflicts, future legs/plans, normal restriction and permissive
updates, and u64 limits. They do not execute a provider or verify signatures.

Baseline source reconstruction matched blob
`0c86d35dff3150f122b3edccbbb03e812ea14b09` (32,060 bytes). Scoped diff and public
surface/source-preservation checks were performed. The local tree is scope-only.

Required exact-head execution:

```text
cargo +1.98.0 check --locked -p search-access --all-targets
cargo +1.98.0 test --locked -p search-access --all-targets
cargo +1.98.0 fmt -p search-access -- --check
cargo +1.98.0 clippy --locked -p search-access --all-targets -- -D warnings
```

Rust compile/tests/rustfmt/Clippy: NOT_RUN; Cargo is absent (attempt exit 127).
No Actions dispatch, self-review, accepted handoff or live-Qdrant qualification.
This repairs the existing compiler, not the outstanding authenticated provider
wiring, policy persistence, signature adapter or canonical daemon cutover.
