# Primitive ownership rules

This file prevents agents from defining local look-alike contracts or hiding mutable state inside a
consumer. Port direction is defined separately in [`PORT_CATALOG.md`](PORT_CATALOG.md).

## Three layers

### 1. `search-contracts` — shared shapes and identities

Owns versioned public records, strong IDs, wire schemas, reason-code registry and canonical
serialization inputs. Other packages may construct or validate these records but do not redeclare them.

Examples: `SourceRevision`, `SourceMembership`, `SearchTaskPlan`, `SearchCandidateSet`,
`ProviderEnvelope`, `QueryExecutionBudget`, `NativeAnchor`, `SearchSourceHandle`.

### 2. `search-domain` — pure reusable meaning

Owns deterministic transition, eligibility, fingerprint, ordering and coverage functions over contract
types. It performs no I/O and persists or supervises no state.

Examples: source-owner transition legality, publication transition legality, eligibility-AST
equivalence, stable candidate ordering and coverage classification.

### 3. Capability package — one mutable-state and side-effect owner

Owns implementation state, commands, receipts and failure handling for one causal capability. Its
public surface uses contract types and package-owned opaque types. Concrete vendor translation remains
inside the corresponding adapter.

| Runtime primitive or state | Sole owner |
|---|---|
| data-root owner guard / owner epoch | `search-runtime-owner` |
| OS-user/incarnation-bound secret references | `search-os-secrets` |
| redb journal transaction and immutable snapshot cache | `search-control-redb` |
| source-admission policy evaluation and decision receipt | `search-source-admission` |
| root, membership, portfolio, source-view and namespace-cutover state | `search-source-registry` |
| physical/logical source identity and path history | `search-source-identity` |
| watcher cursor and reconciliation plan | `search-source-reconcile` |
| stable no-execute read attempt and receipt | `search-safe-reader` |
| residency-aware CAS object, revision lease and exact readback | `search-revision-store` |
| materializer / unitizer / code-enricher profile state | corresponding preparation package |
| lexical profile and sparse encoding | `search-lexical` |
| point canonical key and collision decision | `search-point-identity` |
| projection manifest planning | `search-projection-planner` |
| qualified Qdrant executable and child-process lifecycle | `search-qdrant-supervisor` |
| Qdrant collection, point, query and readback data plane | `search-qdrant-bridge` |
| publication actor, intent recovery and visibility commit | `search-publication` |
| epoch/route pin guards and reclamation watermark | `search-epoch-pins` |
| ordinary retired-point delete plan and receipt | `search-index-reclaimer` |
| grant intersection, live deny and safe retrieval legs | `search-access` |
| saved/unsaved overlay snapshot and shadow set | `search-overlay` |
| exact denominator and execution report | `search-exact` |
| subject ambiguity resolution | `search-subject-resolver` |
| recipe/task-plan compilation | `search-query-planner` |
| bounded leg execution and rank fusion | `search-retrieval-executor` |
| source-backed candidate validation | `search-candidate-validator` |
| cross-repository behavior matrix | `search-comparator` |
| ephemeral/durable source-handle records and expansion authorization | `search-handles` |
| compact result-card projection | `search-result-projector` |
| continuation record, TTL and candidate window | `search-continuation` |
| CAS retention, security purge and restore quarantine | `search-retention` |
| control corpus and Product Pulse verdict | `search-eval` |
| frame, session and binding state | `search-provider-protocol` |

## Critical separations

- `search-source-admission` decides whether an observed source class may be admitted;
  `search-safe-reader` only acquires bytes from an already admitted locator.
- `search-qdrant-supervisor` owns the local process; `search-qdrant-bridge` owns the vendor data plane.
- `search-publication` retires exact point identities; `search-index-reclaimer` deletes them only after
  the pin watermark permits it.
- `search-retention` performs security/legal purge and CAS lifecycle; ordinary index reclamation is not
  accepted as a purge receipt.
- `search-result-projector` selects handle subjects; `search-handles` owns handle state and every
  authorization check; `search-continuation` owns pagination/continuation state only.

## Conflict rule

When two assignments appear to name the same concept:

1. shared serialized shape belongs to `search-contracts`;
2. pure transition or ordering meaning belongs to `search-domain`;
3. mutable state and side effects belong to the sole owner above;
4. vendor translation belongs only to the adapter named in `PORT_CATALOG.md`;
5. a consumer stores only an opaque/reference form unless its assignment explicitly owns the state.

Do not resolve ambiguity by copying a struct or opening another package's store. Submit a contract-change
request naming producer, consumer, required port, field-level need, version impact and owner.
