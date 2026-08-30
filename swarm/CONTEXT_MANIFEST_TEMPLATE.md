# Materialized writer-context rendering guide

This guide is a human review view of `swarm/schemas/context-manifest-v1.toml`. The machine schema is
normative. The integration owner creates the immutable manifest from one non-claimable context draft at
one exact base commit.

Canonical record path:

```text
swarm/context-manifests/<package>/<context_record_sha256>.toml
```

`context_record_sha256` is the external SHA-256 of the complete committed manifest file. It is different
from the embedded signed-payload digest.

## Identity

- `identity.context_id`
- `identity.operation_id`
- `identity.package`
- `identity.stage`
- `identity.wave`
- `identity.base_commit`

## Draft binding

- `draft.path`
- `draft.git_blob_id`
- `draft.exact_sha256`

The draft path and Git blob must exist at the immutable base commit. `draft.exact_sha256` hashes the exact
committed draft bytes.

## Writer-visible artifact

- `artifact.ref`
- `artifact.sha256`
- `artifact.bytes`
- `artifact.format = ELIOT_SWARM_CONTEXT_1`

The schema-level `writer_visible_artifact_count` is one. The artifact is bounded materialized content
derived only from declared sources and fragments. The manifest contains identities and metadata, not
embedded source bodies.

## Source records

- `sources[]`
- `sources[].order`
- `sources[].repository_path`
- `sources[].git_blob_id`
- `sources[].exact_sha256`
- `sources[].exact_bytes`
- `sources[].materialization = UTF8_LF`
- `sources[].materialized_sha256`
- `sources[].materialized_bytes`

Every source preserves declared draft order. The exact committed identity and normalized UTF-8/LF
identity are recorded separately and never compared as though they were the same byte sequence.

## Registry fragments and accepted handoffs

- `registry_fragments[]`
- `accepted_handoffs[]`

Every registry selector resolves exactly once. Every accepted handoff is immutable, declared by the draft
and bound by package, accepted commit, API/configuration/evidence digests and compatibility class.

## Verification

- `verification.source_count`
- `verification.registry_fragment_count`
- `verification.accepted_handoff_count`
- `verification.readback_verified = true`
- `verification.forbidden_path_scan_passed = true`

Counts equal their arrays. Missing blobs, undecodable UTF-8, architecture/dependency implementation
sources and undeclared paths fail closed.

## Signature

- `signature.created_at`
- `signature.materializer_identity`
- `signature.reviewer_identity`
- `signature.record_sha256`

Materializer and reviewer are different actors. `signature.record_sha256` is the signed-payload digest;
`context_record_sha256` is the external complete-file digest used in the path.

Any changed source byte, selector, accepted handoff, ordering or base commit creates a new manifest and
artifact. An acknowledged context is never amended.
