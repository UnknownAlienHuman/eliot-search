# Error and reason-code namespaces

Do not use one global uppercase-string enum for every failure. Four namespaces have different
compatibility and disclosure rules.

## 1. Public provider reasons — `SearchReasonCodeV1`

These may appear in coverage, capability descriptors and provider terminal results. Candidate-level
use is restricted to validated candidates and cannot represent a failed validation.

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

The first group is the S34 key list; the final cross-section codes are required elsewhere by Part I.
`STALE`, `UNREADABLE`, `ACCESS_REVOKED`, `PURGED` and `SOURCE_REVISION_UNAVAILABLE` describe validation
or coverage gaps. They cannot label an emitted evidence candidate.

## 2. Protocol errors — `ProtocolErrorCode`

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

These reject/terminate transport or session operations, not candidate evidence.

## 3. Contract-validation errors — `ContractErrorCode`

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
RECIPE_RESULT_MISMATCH
```

They are programming/input-validation errors and are not automatically disclosed to an untrusted
client.

## 4. Package-local errors

Packages may define internal variants such as `SECRET_LEASE_INVALID`, `RECLAIM_PINNED` or
`SOURCE_ADMISSION_DENIED`. They are not stable provider reasons without an explicit registry mapping.

Every handoff supplies:

| Local error | Public/provider mapping | Protocol mapping | Retryability | Disclosure |
|---|---|---|---|---|

Vendor strings/native numeric codes stay private.

## Mapping rules

- Never map access/purge failure to success or silent omission.
- A contaminated scoring/IDF leg maps to `ACCESS_REVOKED` and is discarded/replanned.
- A resource limit maps to `RESOURCE_EXHAUSTED` or truthful `INCOMPLETE_COVERAGE`.
- A stale/unreadable nomination maps to a `CandidateValidationGap`; material loss also includes
  `INCOMPLETE_COVERAGE`.
- Unknown newer-major codes fail closed; minor extensions require negotiation and explicit
  non-load-bearing classification.
