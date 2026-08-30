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
- `identity.stage`
- `identity.reviewed_at`
- `identity.reviewer`
- `submission.ref`
- `submission.sha256`
- `submission.final_commit`
- `submission.writer`

The reviewer differs from the writer and declares no material conflict.

## Recomputed checks

### Scope

- `scope.complete_diff_digest`
- `scope.write_scope_match`
- `scope.out_of_scope_files[]`

### Contract and public surface

- `contract.primary_contract_digest`
- `contract.current_stage_digest`
- `contract.api_schema_digest`
- `contract.configuration_digest`
- `contract.error_reason_digest`
- `dependencies[]`

### Evidence and size

- `evidence_review.required_results[]`
- `evidence_review.raw_outcomes_reproduced`
- `evidence_review.unavailable_checks[]`
- `size.handwritten_src_lines`
- `size.package_test_lines`
- `size.split_review_status`

Every value is independently recomputed from the exact submission/final commit or remains explicitly
unavailable. The review cannot trust a writer's summary in place of source/evidence readback.

## Findings and verdict

- `findings[]`
- `verdict.decision`
- `verdict.blocking_reason_codes[]`
- `verdict.accepted_submission_sha256`

Allowed decisions are:

```text
ACCEPT_SUBMISSION_FOR_INTEGRATION
REQUEST_CHANGES
REJECT
SUPERSEDED
```

Acceptance requires the exact submission digest, no blocking finding, scope pass, contract pass, evidence
pass and line-budget pass. It permits only the integration owner to construct a package handoff after
final digest verification.

## Signature

- `signature.reviewer_identity`
- `signature.record_sha256`
- `signature.reviewer_signature_ref`

The review remains immutable and append-only. A later correction requires a new review and, where
applicable, a supersession receipt.
