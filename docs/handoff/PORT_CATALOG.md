# Vendor-neutral port catalog

This catalog fixes I/O and state boundaries before implementation. It does not prescribe Rust syntax;
it defines owner, implementation adapter, consumers and forbidden leakage. Shared wire shapes belong to
`search-contracts`; pure reusable meaning belongs to `search-domain`.

## Rules

1. A capability/orchestration package depends on a port contract, not a concrete vendor adapter.
2. Concrete implementations are constructed and bound only by `eliot-searchd`.
3. Vendor types, credentials, raw database handles and process handles never cross a port.
4. A port operation returns typed receipts/reasons and supports the required budget/cancellation model.
5. A package may expose package-owned opaque types, but shared serialized records are not redeclared.
6. Adding a load-bearing port or changing its semantics requires a contract-change receipt and API digest.

## Port ownership matrix

| Port | Contract/meaning owner | Concrete implementation | Main consumers | Must never expose |
|---|---|---|---|---|
| `ControlJournalPort` | `search-contracts` / `search-domain` | `search-control-redb` | publication, registry/runtime, retention, daemon | redb transactions/tables/keys |
| `ControlSnapshotPort` | contracts/domain | `search-control-redb` | access, planner, daemon | mutable redb handles |
| `SecretStorePort` | contracts/domain | `search-os-secrets` | daemon, Qdrant supervisor, provider binding | plaintext secret, DPAPI/vendor blob |
| `ProcessSupervisorPort` | contracts/domain | `search-qdrant-supervisor` | daemon | PID/Job Object/native handles as authority |
| `SearchIndexPort` | contracts/domain | `search-qdrant-bridge` | publication, retrieval executor | qdrant-client request/response types |
| `SearchIndexAdminPort` | contracts/domain | `search-qdrant-bridge` | index reclaimer, purge coordination | broad unbounded delete/filter primitives |
| `SourceAdmissionPort` | contracts/domain | `search-source-admission` | source registry | filesystem/database handles |
| `SourceInventoryPort` | contracts/domain | `search-source-registry` | exact plane, reconciler, planner | mutable registry storage internals |
| `SourceOwnershipPort` | contracts/domain | `search-source-registry` | daemon/adapters/cutover flow | client authority or second catalogue |
| `SafeReaderPort` | contracts/domain | `search-safe-reader` | source spine, reconciler | platform file handles or executable hooks |
| `SourceRevisionStorePort` | contracts/domain | `search-revision-store` | validator, exact, retention, adapters | filesystem paths as authorization |
| `MaterializerPort` | contracts/domain | `search-materializer` | preparation/runtime | provider-native objects |
| `UnitizerPort` | contracts/domain | `search-unitizer` | preparation/overlay | parser-native mutable trees |
| `CodeEnricherPort` | contracts/domain | `search-code-enricher` | preparation/query | compiler certainty not actually provided |
| `LexicalEncoderPort` | contracts/domain | `search-lexical` | projection/executor/overlay | hidden index or corpus storage |
| `ProjectionPlannerPort` | producer public contract | `search-projection-planner` | publication/runtime | Qdrant vendor schema types |
| `EpochPinPort` | producer public contract | `search-epoch-pins` | executor, continuation, reclaimer, retention | durable ordinary-query lease |
| `IndexReclaimerPort` | producer public contract | `search-index-reclaimer` | daemon/retention hardening | purge acknowledgement or broad delete |
| `AccessCompilerPort` | producer public contract | `search-access` | planner, executor, validator, exact | raw vendor filters or client authority |
| `OverlayPort` | producer public contract | `search-overlay` | planner/executor/validator | durable unsaved bytes |
| `ExactScannerPort` | producer public contract | `search-exact` | provider protocol/adapters | indexed top-k denominator |
| `HandleStorePort` | producer public contract | `search-handles` | projector, protocol, retention | raw content/path/vendor cursor in token |
| `ContinuationPort` | producer public contract | `search-continuation` | provider protocol | raw Qdrant cursor or indefinite pin |
| `LifecyclePort` | producer public contract | `search-retention` | daemon/operator commands | physical secure-erase guarantee |
| `ProviderTransportPort` | producer public contract | `search-provider-protocol` | CLI, daemon, leaf adapters | store/index clients or client authority |

## Composition examples

```text
search-publication
  → ControlJournalPort + SearchIndexPort
  → daemon binds search-control-redb + search-qdrant-bridge

search-retrieval-executor
  → IndexQueryPort / DirectLegPort / ProviderLegPort
  → daemon binds the accepted implementations

search-candidate-validator
  → SourceRevisionStorePort + live security state
  → daemon binds search-revision-store

search-retention
  → ControlLifecyclePort + ObjectStoreAdminPort + SearchIndexAdminPort + HandleStorePort
  → daemon binds control journal, revision store, Qdrant bridge and handle owner
```

## Handoff requirement

A producer handoff must publish:

- canonical public API/schema digest;
- operation semantics and idempotency/cancellation behavior;
- typed error/reason mapping;
- concurrency and resource bounds;
- fake/in-memory conformance fixture usable by consumers;
- explicit list of vendor/native types proven absent from the public API.

A consumer may not invent a local substitute port when the producer handoff is missing. It submits a
contract-change request and remains blocked.
