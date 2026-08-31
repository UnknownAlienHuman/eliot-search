# Context artifact candidate digest v1

The builder uses three non-interchangeable digests.

## Artifact digest

```text
artifact_sha256 = SHA-256(exact ELIOT_SWARM_CONTEXT_1 bytes)
```

## Candidate identity

```text
candidate_id = SHA-256(
  ASCII("eliot-search/context-artifact-candidate/v1\0")
  || exact ELIOT_SWARM_CONTEXT_1 bytes
)
```

The domain separator prevents an artifact digest from being reused as candidate identity.

## Candidate metadata digest

```text
candidate_sha256 = SHA-256(
  ASCII("eliot-search/context-artifact-candidate-metadata/v1\0")
  || canonical JSON bytes of the complete candidate object
     with only candidate_sha256 omitted
)
```

Canonical JSON is UTF-8, LF terminated, lexicographically key-sorted and compact. Array order is semantic
and preserved.

Fixed-point hashing, placeholder replacement, omission of any other field and parsed reserialization are
forbidden. These digests are not authoritative context, operation, artifact-store, signature, ticket,
lease, handoff, gate or wave identities.
