# Source evidence validation

Read the [architecture manifesto](../../../docs/architecture/README.md) and
[implementation guide](../../../docs/implementation/README.md#turn-a-point-into-evidence).
[FUNCTIONS.md](FUNCTIONS.md) describes the actual API and its missing production readback connection.

Turn nominated points into exact retained-source evidence or explicit gaps. Resolve the point through
the pinned published manifest and unit/representation records to the actual revision and range.
Qdrant payload, current pathname contents and fixture IDs cannot supply that connection.

Verify real bytes at one clear owner: the retained reader or this validator. The present
`reopen_and_verify` compares supplied digest fields without hashing `readback.bytes`; copying expected
values from the request does not verify anything. Connect actual byte/range checks and the request's
deadline/cancellation with the reader. Keep failed or partial readback out of successful evidence.

Preserve the user's resolved scope, published view, source/profile coordinates and actual removal or
restriction. Selected restricted clients also need real delegated scope and current revocation checks.
Remove redundant grant, fingerprint and residency requirements from the trusted-local API with their
consumers; never replace them with fabricated permissions or disable a real enforced restriction.

Change this owner's types and its immediate query/source/result callers together. There is no separate
contract-change approval, package-only edit wall or line quota. Keep one validation path; do not create
another validator or compatibility wrapper to fit old constructors. Concrete storage/vendor I/O stays
with the existing effect owners; pure validation can be called directly.

Check real revision/byte/range mismatches, missing retained objects, excluded or removed sources,
cancellation and non-exhaustive ranked results through the connected application. Mocks check local
branches; they do not prove source retention or runtime behavior.
