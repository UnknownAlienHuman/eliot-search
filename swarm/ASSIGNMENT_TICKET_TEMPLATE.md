# Assignment ticket rendering guide

This guide is a human review view of `swarm/schemas/assignment-ticket-v1.toml`. The machine schema is
normative. The integration owner creates the immutable record; the package writer never edits it.

Canonical record path:

```text
swarm/tickets/<package>/<ticket_id>.toml
```

## Identity

- `identity.ticket_id`
- `identity.operation_id`
- `identity.package`
- `identity.stage`
- `identity.wave`
- `identity.issued_at`

## Actors

- `actors.writer`
- `actors.reviewer`
- `actors.issuer`

Writer and reviewer must be valid, assigned and distinct. The issuer is the integration owner.

## Repository fence

- `repository_fence.repository`
- `repository_fence.base_commit`
- `repository_fence.branch_or_worktree`
- `repository_fence.write_scope`
- `repository_fence.feature_profile`

The base commit is a full algorithm-tagged Git object ID. The worktree value is opaque and does not
replace the base commit. The write scope exactly matches the function/package registry and cannot be
broadened by this guide.

## Context

- `context.manifest_ref`
- `context.manifest_sha256`
- `context.artifact_ref`
- `context.artifact_sha256`

The context record and one writer-visible artifact must already exist and pass exact readback. A ticket
does not materialize context.

## Instructions

- `instructions[]`

Every `OrderedInstructionDigest` element records declared order, repository path, Git blob ID, exact file
SHA-256 and exact byte length. The array covers all required registry/instruction sources in the declared
context order; local substitutions are invalid.

## Accepted dependency handoffs

- `dependencies[]`

Each element binds the dependency package, immutable handoff record, accepted commit, API/schema digest,
optional configuration digest, evidence digest and compatibility class. Branches and dependency
implementation source are invalid inputs.

## Fixtures and evidence

- `fixtures[]`
- `evidence.required_commands[]`
- `evidence.required_evidence[]`
- `evidence.unavailable_checks[]`

Commands are bounded and include timeout, environment, expected exit class and evidence class.
Unavailable checks remain explicit; compilation cannot substitute for missing execution evidence.

## Limits

- `limits.soft_src_lines`
- `limits.split_review_total_lines`
- `limits.hard_total_lines`
- `limits.static_context_artifacts`

The static context artifact count equals one. The line limits equal or narrow the package registry and
never exceed the repository hard stop.

## Signature

- `signature.record_sha256`
- `signature.integration_signature_ref`

`signature.record_sha256` is the signed-payload digest, not a complete-file self-hash. Complete-file
identity is recorded externally as `exact_record_file_sha256`.

## Non-claims

- the ticket contains no lease ID or lease state;
- ticket presence does not create a writer lease;
- implementation is unauthorized until a separate lease exists and the writer records an
  `ACKNOWLEDGED` lease event;
- the ticket cannot accept the package, a gate or a wave.
