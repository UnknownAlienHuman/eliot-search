# Error and reason-code namespaces

Do not use one global uppercase-string enum for every failure. Four namespaces have different
compatibility and disclosure rules.

## 1. Public provider reasons — `SearchReasonCodeV1`

These may appear in candidate sets, coverage, capability descriptors and provider terminal results.
They are stable and versioned.

```text
QDRANT_UNAVAILABLE
QDRANT_CAPABILITY_MISMATCH
COLLECTION_SCHEMA_MISMATCH
PUBLICATION_BLOCKED
PUBLICATION_READBACK_MISMATCH
POINT_ID_COLLISION
SOURCE_UNSTABLE
OBSERVATION_GAP
SOURCE_REVISION_UNAVAILABLE
SCOPE_CHANGED_OR_REVISION_UNAVAILABLE
REFERENCE_SCOPE_EMPTY
AMBIGUOUS_SUBJECT
UNSAVED_BUFFER_UNOBSERVED
UNSAVED_SNAPSHOT_NOT_ADMITTED
SOURCE_NAMESPACE_OWNERSHIP_CONFLICT
SOURCE_OWNER_CUTOVER_REQUIRED
RESIDENCY_DOMAIN_MISMATCH
CLIENT_ADAPTER_AUTHORITY_VIOLATION
ACCESS_REVOKED
PURGED
SNAPSHOT_EXPIRED
INDEX_GAP
INCOMPLETE_COVERAGE
MATERIALIZATION_LOSS
CONTROL_STORE_CORRUPT
RESTORE_PENDING_REVALIDATION
RESOURCE_EXHAUSTED
CANCELLED
SECURITY_FAIL_CLOSED
STALE
UNREADABLE
```

The first 26 are the S34 key list. The final five are explicitly required elsewhere by Part I and are
included in the closed v1 provider registry.

## 2. Protocol errors — `ProtocolErrorCode`

These terminate or reject transport/session operations and are not candidate reasons:

```text
PROTOCOL_VERSION_MISMATCH
FRAME_TOO_LARGE
INVALID_ENVELOPE
REPLAY_DETECTED
AUTH_FAILED
BINDING_MISMATCH
SEQUENCE_GAP
IN_FLIGHT_LIMIT_EXCEEDED
DEADLINE_EXPIRED
UNSUPPORTED_MESSAGE_KIND
```

A protocol error may carry a mapped public reason only when product coverage is also affected.

## 3. Contract-validation errors — `ContractErrorCode`

These are programming/input validation errors:

```text
EPOCH_OUT_OF_RANGE
EPOCH_EXHAUSTED
UNKNOWN_LOAD_BEARING_FIELD
CONTRACT_VERSION_MISMATCH
INVALID_CONTRACT_SHAPE
CANONICALIZATION_FAILED
DIGEST_MISMATCH
BOUND_EXCEEDED
INVALID_TAGGED_VARIANT
```

They are not automatically exposed to an untrusted client.

## 4. Package-local errors

Each package may define internal variants such as `SECRET_LEASE_INVALID`, `RECLAIM_PINNED` or
`SOURCE_ADMISSION_DENIED`. They are not stable provider reason codes unless this registry adds an
explicit mapping.

Every package handoff supplies a table:

| Local error | Public/provider mapping | Protocol mapping | Retryability | Disclosure |
|---|---|---|---|---|

Vendor error strings and native numeric codes stay private. Default logs contain only the local typed
code, operation ID and bounded non-content metadata.

## Mapping rules

- Never map an access/purge failure to a generic success or candidate omission.
- A contaminated scoring/IDF leg maps to `ACCESS_REVOKED` and is discarded/replanned.
- A bounded resource limit maps to `RESOURCE_EXHAUSTED` or truthful `INCOMPLETE_COVERAGE`.
- A stale/unreadable candidate maps to `STALE`/`UNREADABLE`; material coverage loss also includes
  `INCOMPLETE_COVERAGE`.
- Internal corruption maps to the narrow public reason (`CONTROL_STORE_CORRUPT`, schema mismatch, etc.)
  only after content-minimized classification.
- Unknown codes from a newer major version fail closed; unknown minor extensions are accepted only when
  negotiated and explicitly non-load-bearing.
