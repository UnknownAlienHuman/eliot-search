# Package submission rendering guide

This guide is a human review view of `swarm/schemas/package-submission-v1.toml`. A writer produces the
package-only commit and evidence; the integration owner records the immutable submission. The machine
schema is normative.

Canonical record path:

```text
swarm/submissions/<package>/<submission_id>.toml
```

## Identity and authority chain

- `identity.submission_id`
- `identity.operation_id`
- `identity.package`
- `identity.stage`
- `identity.submitted_at`
- `ticket_lease_context.ticket_ref`
- `ticket_lease_context.ticket_exact_record_file_sha256`
- `ticket_lease_context.lease_ref`
- `ticket_lease_context.lease_exact_record_file_sha256`
- `ticket_lease_context.context_manifest_ref`
- `ticket_lease_context.context_manifest_exact_record_file_sha256`

Every record digest is the SHA-256 of the complete committed file and equals the exact digest inside its
immutable ref. None is an embedded `signature.record_sha256`. The lease must be acknowledged, active and
non-superseded.

## Repository and complete diff

- `repository.base_commit`
- `repository.final_commit`
- `repository.branch_or_worktree`
- `repository.write_scope`
- `changed_files[]`

Every changed file includes path, closed status, old blob and new blob wrappers. The list is complete,
sorted, unique and wholly inside the leased package scope.

## Public handoff candidate

- `public_handoff_candidate.api_manifest_ref`
- `public_handoff_candidate.api_schema_digest`
- `public_handoff_candidate.configuration_digest`
- `public_handoff_candidate.fixture_digest_set`
- `public_handoff_candidate.error_reason_digest`
- `public_handoff_candidate.compatibility`

Configuration absence is explicit `OptionalV1` `ABSENT`; TOML `null`, omission and sentinel strings are
invalid.

## Commands and evidence

- `command_outcomes[]`
- `evidence.required_results[]`
- `evidence.unavailable_checks[]`
- `evidence.contract_change_refs[]`

Raw output remains in immutable artifacts. An unavailable check remains visible and cannot be inferred
from another command.

## Size and residual state

- `size.handwritten_src_lines`
- `size.package_test_lines`
- `size.split_review_required`
- `size.split_review_ref`
- `residual_state.known_failures[]`
- `residual_state.deferred_nonowned_work[]`
- `residual_state.no_placeholder_success_attestation = true`

## Signature and non-claims

- `signature.writer_identity`
- `signature.record_sha256`
- `signature.writer_signature_ref`

The submission is independent-review input only. It does not accept the package, publish a handoff,
satisfy a gate or advance a wave.
