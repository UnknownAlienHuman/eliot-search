# Accepted evidence digest v1

`OrderedAcceptedPackageHandoff.evidence_digest` is a semantic digest derived from the exact accepted
`package_handoff_v1` record. It is not an extra mutable field in that handoff and is not record-path
identity.

Machine profiles:

- `swarm/accepted-evidence-digest-v1.toml`;
- `swarm/type-rule-profiles-v1.toml`.

## Input

Read and verify the immutable handoff reference first: repository, commit, path, Git blob ID, exact full
record SHA-256, signed-payload digest and signature. Then consume the committed `evidence[]` array in its
exact array order.

Each element has exactly:

```text
requirement_id
evidence_class
artifact_ref
artifact_sha256
raw_outcome_digest
availability
```

`requirement_id` is unique and `artifact_sha256` equals `artifact_ref.sha256`.

## Canonical bytes

```text
ELIOT_ACCEPTED_EVIDENCE_MANIFEST_1<LF>
<canonical compact JSON for evidence[0]><LF>
<canonical compact JSON for evidence[1]><LF>
...
```

JSON object keys are lexicographically sorted, UTF-8 is strict, line endings are LF, null/floating-point
values and unknown fields are rejected. Array order is preserved rather than silently sorted.

The digest is SHA-256 over all manifest bytes, including the magic line and terminal LF. Empty evidence
is valid and hashes the magic line alone.

## Boundary

This digest is a convenience identity for the reviewed evidence set. The `handoff_ref` remains the
immutable authority and transitively binds the full handoff, evidence, compatibility and signatures.
The evidence digest cannot replace exact handoff readback, cannot name a control record and cannot accept
a package, gate or wave.
