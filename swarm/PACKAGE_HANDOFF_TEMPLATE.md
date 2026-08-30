# Accepted package handoff rendering guide

This guide is a human review view of `swarm/schemas/package-handoff-v1.toml`. The machine schema is
normative. Only the integration owner publishes the immutable record after an accepted independent
review.

Canonical record path:

```text
swarm/handoffs/<package>/<handoff_id>.toml
```

`handoff_id` is unique record/path identity. `api_schema_digest` is public-surface identity and is not
used as the filename.

## Identity

- `identity.handoff_id`
- `identity.operation_id`
- `identity.package`
- `identity.stage`
- `identity.accepted_at`

## Submission and review

- `submission_review.submission_ref`
- `submission_review.submission_exact_record_file_sha256`
- `submission_review.review_ref`
- `submission_review.review_exact_record_file_sha256`

Both digests identify complete committed control-record files and equal the exact digests inside their
immutable refs. Neither is the referenced record's embedded signed-payload digest. The review has verdict
`ACCEPT_SUBMISSION_FOR_INTEGRATION`, binds the exact submission and has no unresolved blocking finding.

## Accepted code

- `accepted_code.base_commit`
- `accepted_code.final_commit`
- `accepted_code.changed_files_digest`

## Public surface

- `public_surface.api_manifest_ref`
- `public_surface.api_schema_digest`
- `public_surface.configuration_digest`
- `public_surface.fixture_digest_set`
- `public_surface.error_reason_digest`

Configuration absence is explicit `OptionalV1` `ABSENT`.

## Dependencies and evidence

- `dependencies[]`
- `evidence[]`

Every dependency is an accepted immutable package handoff. Every evidence entry binds an immutable
artifact, artifact digest, raw-outcome digest and availability state.

## Compatibility

- `compatibility.class`
- `compatibility.consumer_actions[]`

Compatibility actions use the closed `ConsumerActionCode` registry. Free-form hidden migration
instructions are invalid.

## Supersession and signature

- `supersession.supersedes_handoff_ref`
- `supersession.supersedes_handoff_exact_record_file_sha256`
- `signature.integration_owner_identity`
- `signature.record_sha256`
- `signature.integration_signature_ref`

A correction creates a new handoff and append-only supersession receipt. Existing accepted records are
never edited. The optional superseded-handoff digest is a complete-file digest, never a signed-payload
digest.

## Non-claims

A package handoff accepts one package implementation/public surface only. It does not accept G0–G6,
Product Pulse, a wave, optional depth or launch-state advancement.
