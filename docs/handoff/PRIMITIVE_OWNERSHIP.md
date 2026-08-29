# Primitive ownership rules

This file prevents agents from defining local look-alike contracts.

## Three layers

### 1. `search-contracts` — shared shapes and identities

Owns versioned public records, strong IDs, wire schemas, reason-code registry and canonical
serialization inputs. Other packages may construct or validate these records but do not redeclare them.

Examples: `SourceRevision`, `SourceMembership`, `SearchTaskPlan`, `SearchCandidateSet`,
`ProviderEnvelope`, `QueryExecutionBudget`, `NativeAnchor`, `SearchSourceHandle`.

### 2. `search-domain` — pure reusable meaning

Owns deterministic transition/eligibility/fingerprint/ordering/coverage functions over contract types.
It does not persist or supervise the state.

Examples: source-owner transition legality, publication transition legality, eligibility AST
equivalence, stable candidate ordering and coverage classification.

### 3. Capability package — owned runtime state and behavior

Owns process-local or adapter-specific implementation state, commands, receipts and failure handling for
one causal capability. Its public surface uses contract types and package-owned opaque handles; it never
creates a duplicate shared wire record.

Examples:

| Runtime primitive | Owner |
|---|---|
| data-root owner guard/lease | `search-runtime-owner` |
| redb journal transaction/snapshot cache | `search-control-redb` |
| root/membership/portfolio registry state | `search-source-registry` |
| watcher cursor/reconcile plan | `search-source-reconcile` |
| stable read attempt/receipt | `search-safe-reader` |
| CAS object address/retention lease/readback | `search-revision-store` |
| materializer/unitizer/enricher profile state | corresponding preparation package |
| point canonical key/collision decision | `search-point-identity` |
| projection manifest planning | `search-projection-planner` |
| Qdrant vendor request/ack/readback | `search-qdrant-bridge` |
| publication actor/intent recovery | `search-publication` |
| epoch/route pin guard | `search-epoch-pins` |
| grant intersection/live deny/safe leg | `search-access` |
| overlay snapshot/shadow set | `search-overlay` |
| exact denominator/execution | `search-exact` |
| subject ambiguity resolution | `search-subject-resolver` |
| recipe plan compilation | `search-query-planner` |
| bounded leg execution/fusion | `search-retrieval-executor` |
| source-backed candidate validation | `search-candidate-validator` |
| cross-repository behavior matrix | `search-comparator` |
| compact result card projection | `search-result-projector` |
| continuation record/TTL/window | `search-continuation` |
| sweep/purge/restore execution | `search-retention` |
| control corpus/Product Pulse verdict | `search-eval` |
| frame/session/binding state | `search-provider-protocol` |

## Conflict rule

When two assignments appear to name the same concept:

1. shared serialized shape belongs to `search-contracts`;
2. pure transition/ordering meaning belongs to `search-domain`;
3. mutable runtime state and side effects belong to the capability package;
4. vendor translation belongs only to the adapter;
5. the consumer stores only an opaque/reference form unless its assignment explicitly owns the state.

Do not resolve an ambiguity by copying a struct. Submit a contract-change request naming producer,
consumer, field-level need, version impact and owner.
