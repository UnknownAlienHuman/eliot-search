# Independent package review rendering guide

This guide is a human review view of `swarm/schemas/independent-review-v1.toml`. The machine schema is
normative. One review binds one exact submission and cannot create a handoff, accept a gate or advance
launch state.

Canonical record path:

```text
swarm/reviews/<package>/<review_id>.toml
```

## Identity and submission binding

- `identity.review_id`
- `identity.operation_id`
- `identity.package`
- `identity.reviewed_at`
- `submission.ref`
- `submission.sha256`

The submission ref is an immutable `package_submission_v1` record and the SHA-256 is its external exact
complete-file digest.

## Actors and independence

- `actors.writer`
- `actors.reviewer`
- `actors.independence_declaration = NO_CONFLICT_DECLARED`

The writer matches the submission/ticket. The reviewer is different from the writer and matches the
ticket or an integration-owned reviewer-replacement record.

## Scope review

- `scope_review.complete_diff_recomputed`
- `scope_review.all_paths_inside_write_scope`
- `scope_review.base_final_commit_relationship_verified`

All three values must be true for acceptance.

## Contract review

- `contract_review.primary_contract_satisfied`
- `contract_review.stage_obligations_satisfied`
- `contract_review.dependency_handoffs_match`
- `contract_review.ownership_boundaries_preserved`

All four values must be true for acceptance.

## Evidence review

- `evidence_review.raw_outcomes_reproduced`
- `evidence_review.failure_cancellation_recovery_checked`
- `evidence_review.security_content_audit_checked`
- `evidence_review.unavailable_checks_visible`

`raw_outcomes_reproduced` uses the exact closed PASS/UNAVAILABLE/FAIL-with-finding semantics declared by
the schema. Unavailable checks remain visible and cannot be converted into a pass.

## API and size review

- `api_review.recomputed_api_schema_digest`
- `api_review.recomputed_configuration_digest`
- `api_review.compatibility_class`
- `api_review.line_budget_satisfied`

The configuration digest uses explicit `OptionalV1`. Digests and compatibility are independently
recomputed from the exact submission/final commit.

## Findings and verdict

- `findings[]`
- `verdict.value`
- `verdict.reason_codes[]`

Allowed verdicts are:

```text
ACCEPT_SUBMISSION_FOR_INTEGRATION
REQUEST_CHANGES
REJECT
SUPERSEDED
```

Acceptance requires no unresolved blocker or critical finding and every mandatory scope, contract,
evidence and line-budget condition. It permits only the integration owner to construct a package handoff
after final digest verification.

## Signature

- `signature.record_sha256`
- `signature.reviewer_signature_ref`

The signature binds the reviewer identity. The embedded digest is the signed-payload digest; complete-file
identity remains external.

The review is immutable and append-only. A correction requires a new review and, where applicable, a
supersession receipt.
