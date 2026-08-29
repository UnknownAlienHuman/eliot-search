# Primitive ownership rules

This file prevents local look-alike records, traits and mutable state.

## Four layers

### 1. `search-contracts` — serialized shapes and identities

Owns strong IDs, tagged public records, wire schemas, reason registries, bounds and canonicalization
inputs. Other packages do not redeclare them.

### 2. `search-domain` — pure reusable meaning

Owns deterministic transition, eligibility, fingerprint, ordering and coverage rules over contract
types. It performs no I/O and owns no mutable capability state.

### 3. `search-ports` — shared vendor-neutral operations

Owns trait contracts, operation contexts, idempotency/cancellation/deadline semantics and conformance
fake interfaces. It depends only on `search-contracts` and implements no adapter.

### 4. Capability package — one mutable-state and side-effect owner

Owns commands, private state, receipts and failures for one causal responsibility. Concrete vendor
translation stays inside the corresponding adapter; daemon composition connects accepted ports.

| Runtime primitive or state | Sole owner |
|---|---|
| data-root owner guard / owner epoch | `search-runtime-owner` |
| OS-user/incarnation-bound secret records | `search-os-secrets` |
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
| source-handle records and expansion authorization | `search-handles` |
| compact result-card projection | `search-result-projector` |
| continuation record, TTL and candidate window | `search-continuation` |
| CAS retention, security purge and restore quarantine | `search-retention` |
| control corpus and Product Pulse verdict | `search-eval` |
| frame, session and binding state | `search-provider-protocol` |

## Conflict rule

When two files appear to name the same concept:

1. serialized shape → `search-contracts`;
2. pure meaning → `search-domain`;
3. shared operation trait → `search-ports`;
4. mutable state/side effect → the sole capability owner above;
5. vendor translation → the adapter implementing the accepted port;
6. consumer stores only an opaque/reference form unless its assignment owns the state.

Do not resolve ambiguity by copying a struct or trait. Raise a contract/port change naming producer,
consumer, field/method need, compatibility impact and owner.
