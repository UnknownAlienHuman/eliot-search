# Vendor-neutral port catalog

`search-ports` owns the shared trait surface. This document maps those traits to concrete
implementations and capability consumers; exact operation semantics are frozen in
[`../contracts/p00/PORT_OPERATIONS.md`](../contracts/p00/PORT_OPERATIONS.md).

## Rules

1. Shared serialized records come from `search-contracts`.
2. Shared vendor-neutral traits and operation contexts come from `search-ports`.
3. Pure transition, ordering and coverage meaning comes from `search-domain`.
4. A capability package owns its mutable state and implements or consumes accepted ports.
5. Concrete adapters are constructed only by `eliot-searchd`.
6. Vendor/native types, credentials, raw database handles, process handles and reusable authorization
   decisions never cross a public port.
7. Port changes require an accepted contract-change receipt and a new public API digest.

## Port ownership matrix

| Port | Trait owner | Concrete implementation | Principal consumers |
|---|---|---|---|
| `ClockPort` | `search-ports` | daemon/platform adapter | contracts requiring controlled time, expiry and tests |
| `SecretStorePort` | `search-ports` | `search-os-secrets` | daemon composition, provider binding |
| `ProcessSupervisorPort` | `search-ports` | `search-qdrant-supervisor` | daemon composition |
| `ControlJournalPort` | `search-ports` | `search-control-redb` | publication, registry, retention, daemon |
| `ControlSnapshotPort` | `search-ports` | `search-control-redb` | access, planner, daemon |
| `SourceAdmissionPort` | `search-ports` | `search-source-admission` | source registry |
| `SourceInventoryPort` | `search-ports` | `search-source-registry` | exact, reconciler, planner |
| `SourceOwnershipPort` | `search-ports` | `search-source-registry` | daemon and cutover/adapters |
| `SafeReaderPort` | `search-ports` | `search-safe-reader` | source spine, reconciler |
| `SourceRevisionStorePort` | `search-ports` | `search-revision-store` | validator, exact, retention, adapters |
| `ResidencyPolicyPort` | `search-ports` | selected residency implementation in source/runtime composition | revision store, retention |
| `MaterializerPort` | `search-ports` | `search-materializer` | preparation/runtime |
| `UnitizerPort` | `search-ports` | `search-unitizer` | preparation/overlay |
| `CodeEnricherPort` | `search-ports` | `search-code-enricher` | preparation/query |
| `LexicalEncoderPort` | `search-ports` | `search-lexical` | projection/executor/overlay |
| `ModelProviderPort` | `search-ports` | `search-model-provider` when admitted | optional model worker/executor |
| `SearchIndexPort` | `search-ports` | `search-qdrant-bridge` | publication, retrieval executor |
| `SearchIndexAdminPort` | `search-ports` | `search-qdrant-bridge` | index reclaimer, purge coordination |
| `EpochPinPort` | `search-ports` | `search-epoch-pins` | executor, continuation, reclaimer, retention |
| `AccessCompilerPort` | `search-ports` | `search-access` | planner, executor, validator, exact |
| `OverlayPort` | `search-ports` | `search-overlay` | planner, executor, validator |
| `ExactScannerPort` | `search-ports` | `search-exact` | provider protocol and adapters |
| `HandleStorePort` | `search-ports` | `search-handles` | projector, protocol, retention |

Package-specific orchestration APIs such as projection planning, publication coordination,
continuation and lifecycle may be public capability APIs rather than shared infrastructure traits.
They still use `search-contracts` records and cannot expose their private adapter state.

## Composition examples

```text
search-publication
  → ControlJournalPort + SearchIndexPort
  → daemon binds search-control-redb + search-qdrant-bridge

search-retrieval-executor
  → SearchIndexPort + AccessCompilerPort + EpochPinPort
  → daemon binds accepted implementations

search-candidate-validator
  → SourceRevisionStorePort + AccessCompilerPort
  → daemon binds search-revision-store + search-access

search-retention
  → ControlJournalPort + SourceRevisionStorePort + SearchIndexAdminPort + HandleStorePort
  → daemon binds control, revision, index and handle owners
```

## Handoff requirement

The `search-ports` handoff publishes:

- canonical public API digest and complete method inventory;
- cancellation, deadline, idempotency and bounded-output semantics;
- typed error mapping and retryability;
- fake/in-memory conformance interfaces;
- proof that no vendor/native type crosses the API.

Concrete adapter handoffs publish conformance results against that exact port digest. A consumer cannot
invent a local substitute when a required port is missing; it remains blocked and raises a contract
change.
