# Materialized writer-context rendering guide

This guide is a human review view of `swarm/schemas/context-manifest-v1.toml`. The machine schema is
normative. The integration owner creates the immutable manifest from one non-claimable context draft at
one exact base commit.

Canonical record path:

```text
swarm/context-manifests/<package>/<context_record_sha256>.toml
```

## Identity and draft binding

- `identity.context_id`
- `identity.operation_id`
- `identity.package`
- `identity.stage`
- `identity.base_commit`
- `draft.path`
- `draft.git_blob_id`
- `draft.exact_file_sha256`

## Writer-visible artifact

- `artifact.ref`
- `artifact.sha256`
- `artifact.bytes`
- `artifact.count = 1`
- `artifact.format = ELIOT_WRITER_CONTEXT_V1`

The artifact is bounded materialized content derived only from declared sources and fragments. The
manifest contains identities and metadata, not embedded source bodies.

## Source records

Each `sources[]` element records:

- declared order;
- repository-relative source path;
- exact Git blob ID;
- exact committed SHA-256 and byte length;
- materialization mode `UTF8_LF`;
- normalized SHA-256 and byte length.

Undecodable UTF-8, missing blobs, forbidden paths and undeclared sources fail closed.

## Registry fragments

Each `registry_fragments[]` element records:

- declared order;
- registry path and closed selector;
- source Git blob and exact source digest;
- selector match count exactly one;
- fragment digest and length.

## Accepted handoffs

Each `accepted_handoffs[]` element is an immutable accepted package handoff. Mutable branches,
implementation source and unaccepted API sketches are invalid.

## Verification and signature

- `verification.materializer`
- `verification.independent_reviewer`
- `verification.created_at`
- `verification.forbidden_path_scan`
- `verification.source_count`
- `verification.fragment_count`
- `verification.handoff_count`
- `signature.record_sha256`
- `signature.materializer_signature_ref`
- `signature.reviewer_signature_ref`

Materializer and independent reviewer are different actors. `signature.record_sha256` is the
signed-payload digest; `context_record_sha256` is the external complete-file digest used in the path.

Any changed source byte, selector, accepted handoff, ordering or base commit creates a new manifest and
artifact. An acknowledged context is never amended.
