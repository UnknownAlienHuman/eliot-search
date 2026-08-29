# `search-ports` implementation packet

**Path:** `crates/search-ports`  
**Capability:** C00 vendor-neutral port boundary  
**Delivery:** W0 / P00  
**Gate:** CONDITIONAL on the accepted `search-contracts` schema/API digest  
**Trace:** S4-S5, S31, H4, P00  
**Direct public handoffs:** `search-contracts`

Apply `../ASSIGNMENT_PROTOCOL.md`. Exact operation inventory is in
`docs/contracts/p00/PORT_OPERATIONS.md`.

## Mission

Publish the complete vendor-neutral port surface used by capability packages and concrete adapters,
without choosing an executor, database, OS API or vendor client.

## Logical primitives

- `OperationContext`: request identity, relative deadline, cancellation capability and bounded budget ref
- `MutationIdentity`: operation identity and retry/idempotency class
- `PortReceipt`: operation identity, dependency generation, bounded outcome metadata and retryability
- `BoundedPage<T>`, `BoundedStream<T>`, `ContinuationRef`
- opaque guards/leases for secrets, process ownership, pins and retained revisions

These are interface-support types. Shared serialized wire/domain records remain in `search-contracts`.

## Required port groups

1. `ClockPort`, `SecretStorePort`, `ProcessSupervisorPort`
2. `ControlJournalPort`, `ControlSnapshotPort`
3. `SourceAdmissionPort`, `SourceInventoryPort`, `SourceOwnershipPort`
4. `SafeReaderPort`, `SourceRevisionStorePort`, `ResidencyPolicyPort`
5. `MaterializerPort`, `UnitizerPort`, `CodeEnricherPort`, `LexicalEncoderPort`, optional `ModelProviderPort`
6. `SearchIndexPort`, `SearchIndexAdminPort`, `EpochPinPort`
7. `AccessCompilerPort`, `OverlayPort`, `ExactScannerPort`, `HandleStorePort`

## Invariants

- public ports contain no redb/Qdrant/Windows/parser/model/client-vendor types
- concrete adapters are constructed only by `eliot-searchd`
- a read operation cannot require a durable idempotency row
- mutating retries are keyed by an explicit bounded operation identity
- cancellation/deadline applies to every potentially blocking operation
- a bounded stream cannot silently become an unbounded collection
- a handle, secret lease, process guard or pin grants only its documented capability
- errors are typed and mapped; no adapter string becomes a public reason code directly

## Test and handoff evidence

- `public_vendor_type_guard`
- `port_operation_semantics_complete`
- `fake_timeout_and_cancellation_conformance`
- `fake_partial_receipt_conformance`
- `read_port_has_no_durable_idempotency_requirement`
- `mutation_retry_identity_required`
- canonical public API digest and method inventory

## Suggested modules

```text
search-ports/src/
  context.rs
  receipt.rs
  clock.rs
  control.rs
  secret.rs
  process.rs
  source.rs
  revision.rs
  preparation.rs
  index.rs
  access.rs
  overlay.rs
  exact.rs
  handles.rs
  error.rs
```

## Size / split

- Initial `src/` target: **≤ 5,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local conformance tests**.
- Split only on an independently versioned compatibility boundary; do not create one crate per trait.
