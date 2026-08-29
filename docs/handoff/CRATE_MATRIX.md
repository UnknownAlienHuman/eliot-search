# Crate ownership matrix

This matrix is the human view of `swarm/crates.toml`. One writer owns one Cargo package. Family
directories are navigation only. `wave` is the earliest possible launch; current authorization comes
only from `swarm/launch-state.toml`.

- **Library packages:** 39
- **Binary packages:** 4
- **Total Cargo packages:** 43
- **Mandatory split review:** before 8,500 hand-written Rust lines
- **Hard stop:** 10,000 hand-written Rust lines including package-local tests

| Package | Cell / support boundary | Family | Earliest wave | Target `src/` | Direct dependencies | Assignment |
|---|---|---|---:|---:|---|---|
| `search-contracts` | C00 | foundation | W0 | ≤7,500 | — | [`search-contracts.md`](../../swarm/assignments/search-contracts.md) |
| `search-domain` | shared pure kernel | foundation | W0 | ≤7,000 | `search-contracts` | [`search-domain.md`](../../swarm/assignments/search-domain.md) |
| `search-runtime-owner` | C01 | runtime | W1 | ≤4,500 | `search-contracts`, `search-domain` | [`search-runtime-owner.md`](../../swarm/assignments/search-runtime-owner.md) |
| `search-os-secrets` | C01/C15/C30 security support | runtime | W1 | ≤3,500 | `search-contracts`, `search-domain` | [`search-os-secrets.md`](../../swarm/assignments/search-os-secrets.md) |
| `search-control-redb` | C02 | control | W1 | ≤7,500 | `search-contracts`, `search-domain` | [`search-control-redb.md`](../../swarm/assignments/search-control-redb.md) |
| `search-source-admission` | C03/C06 security support | source | W2 | ≤4,500 | `search-contracts`, `search-domain` | [`search-source-admission.md`](../../swarm/assignments/search-source-admission.md) |
| `search-source-registry` | C03 | source | W2 | ≤6,500 | `search-contracts`, `search-domain`, `search-source-identity`, `search-source-admission` | [`search-source-registry.md`](../../swarm/assignments/search-source-registry.md) |
| `search-source-identity` | C04 | source | W2 | ≤6,500 | `search-contracts`, `search-domain` | [`search-source-identity.md`](../../swarm/assignments/search-source-identity.md) |
| `search-source-reconcile` | C05 | source | W5 | ≤7,000 | `search-contracts`, `search-domain`, `search-source-registry`, `search-source-identity`, `search-safe-reader` | [`search-source-reconcile.md`](../../swarm/assignments/search-source-reconcile.md) |
| `search-safe-reader` | C06 | source | W2 | ≤6,500 | `search-contracts`, `search-domain` | [`search-safe-reader.md`](../../swarm/assignments/search-safe-reader.md) |
| `search-revision-store` | C07 | source | W2 | ≤7,500 | `search-contracts`, `search-domain` | [`search-revision-store.md`](../../swarm/assignments/search-revision-store.md) |
| `search-materializer` | C08 | preparation | W2 | ≤7,000 | `search-contracts`, `search-domain` | [`search-materializer.md`](../../swarm/assignments/search-materializer.md) |
| `search-unitizer` | C09 | preparation | W2 | ≤6,500 | `search-contracts`, `search-domain` | [`search-unitizer.md`](../../swarm/assignments/search-unitizer.md) |
| `search-code-enricher` | C10 | preparation | W5 | ≤7,500 | `search-contracts`, `search-domain` | [`search-code-enricher.md`](../../swarm/assignments/search-code-enricher.md) |
| `search-lexical` | C11 | provider | W3 | ≤7,500 | `search-contracts`, `search-domain` | [`search-lexical.md`](../../swarm/assignments/search-lexical.md) |
| `search-model-provider` | C12 | provider | W10 | ≤6,500 | `search-contracts`, `search-domain` | [`search-model-provider.md`](../../swarm/assignments/search-model-provider.md) |
| `search-projection-planner` | C13 | index | W3 | ≤7,000 | `search-contracts`, `search-domain`, `search-point-identity` | [`search-projection-planner.md`](../../swarm/assignments/search-projection-planner.md) |
| `search-point-identity` | C14 | index | W3 | ≤4,500 | `search-contracts`, `search-domain` | [`search-point-identity.md`](../../swarm/assignments/search-point-identity.md) |
| `search-qdrant-supervisor` | C01/C15 process support | index | W3 | ≤5,500 | `search-contracts`, `search-domain`, `search-os-secrets` | [`search-qdrant-supervisor.md`](../../swarm/assignments/search-qdrant-supervisor.md) |
| `search-qdrant-bridge` | C15 data plane | index | W3 | ≤7,000 | `search-contracts`, `search-domain` | [`search-qdrant-bridge.md`](../../swarm/assignments/search-qdrant-bridge.md) |
| `search-publication` | C16 | index | W3 | ≤7,500 | `search-contracts`, `search-domain`, `search-projection-planner`, `search-point-identity` | [`search-publication.md`](../../swarm/assignments/search-publication.md) |
| `search-epoch-pins` | C17 pin registry | index | W3 | ≤4,500 | `search-contracts`, `search-domain` | [`search-epoch-pins.md`](../../swarm/assignments/search-epoch-pins.md) |
| `search-index-reclaimer` | C17 reclaim executor | index | W3 | ≤4,500 | `search-contracts`, `search-domain`, `search-epoch-pins` | [`search-index-reclaimer.md`](../../swarm/assignments/search-index-reclaimer.md) |
| `search-access` | C18 | query | W4 | ≤7,500 | `search-contracts`, `search-domain` | [`search-access.md`](../../swarm/assignments/search-access.md) |
| `search-overlay` | C19 | query | W5 | ≤7,500 | `search-contracts`, `search-domain`, `search-unitizer`, `search-lexical` | [`search-overlay.md`](../../swarm/assignments/search-overlay.md) |
| `search-exact` | C20 | query | W6 | ≤7,500 | `search-contracts`, `search-domain`, `search-access` | [`search-exact.md`](../../swarm/assignments/search-exact.md) |
| `search-subject-resolver` | C21 | query | W6 | ≤6,000 | `search-contracts`, `search-domain` | [`search-subject-resolver.md`](../../swarm/assignments/search-subject-resolver.md) |
| `search-query-planner` | C22 | query | W4 | ≤7,500 | `search-contracts`, `search-domain`, `search-access` | [`search-query-planner.md`](../../swarm/assignments/search-query-planner.md) |
| `search-retrieval-executor` | C23 | query | W4 | ≤7,500 | `search-contracts`, `search-domain`, `search-query-planner`, `search-lexical`, `search-epoch-pins`, `search-access` | [`search-retrieval-executor.md`](../../swarm/assignments/search-retrieval-executor.md) |
| `search-candidate-validator` | C24 | query | W4 | ≤7,500 | `search-contracts`, `search-domain`, `search-access` | [`search-candidate-validator.md`](../../swarm/assignments/search-candidate-validator.md) |
| `search-comparator` | C25 | query | W6 | ≤7,500 | `search-contracts`, `search-domain`, `search-subject-resolver` | [`search-comparator.md`](../../swarm/assignments/search-comparator.md) |
| `search-handles` | C26/C27 handle support | query | W4 | ≤6,500 | `search-contracts`, `search-domain` | [`search-handles.md`](../../swarm/assignments/search-handles.md) |
| `search-result-projector` | C26 | query | W4 | ≤7,000 | `search-contracts`, `search-domain`, `search-candidate-validator`, `search-handles` | [`search-result-projector.md`](../../swarm/assignments/search-result-projector.md) |
| `search-continuation` | C27 | query | W4 | ≤6,000 | `search-contracts`, `search-domain`, `search-query-planner`, `search-access`, `search-epoch-pins` | [`search-continuation.md`](../../swarm/assignments/search-continuation.md) |
| `search-retention` | C28 | runtime | W7 | ≤7,500 | `search-contracts`, `search-domain`, `search-epoch-pins`, `search-index-reclaimer`, `search-handles` | [`search-retention.md`](../../swarm/assignments/search-retention.md) |
| `search-eval` | C29 | evaluation | W4 | ≤7,500 | `search-contracts`, `search-domain` | [`search-eval.md`](../../swarm/assignments/search-eval.md) |
| `search-provider-protocol` | C30 generic edge | adapter | W1 | ≤7,500 | `search-contracts`, `search-domain` | [`search-provider-protocol.md`](../../swarm/assignments/search-provider-protocol.md) |
| `search-eliot-adapter` | C30 optional ELIOT profile | adapter | W8 | ≤5,500 | `search-contracts`, `search-domain`, `search-provider-protocol` | [`search-eliot-adapter.md`](../../swarm/assignments/search-eliot-adapter.md) |
| `search-research-export-adapter` | C30 optional Research profile | adapter | W8 | ≤6,000 | `search-contracts`, `search-domain`, `search-provider-protocol` | [`search-research-export-adapter.md`](../../swarm/assignments/search-research-export-adapter.md) |
| `eliot-searchd` | composition | binary | W1 | ≤6,500 | progressive accepted capability graph | [`eliot-searchd.md`](../../swarm/assignments/eliot-searchd.md) |
| `eliot-search` | composition | binary | W1 | ≤4,500 | `search-contracts`, `search-provider-protocol` | [`eliot-search.md`](../../swarm/assignments/eliot-search.md) |
| `eliot-search-model-worker` | composition | binary | W10 | ≤4,500 | `search-contracts`, `search-provider-protocol`, `search-model-provider` | [`eliot-search-model-worker.md`](../../swarm/assignments/eliot-search-model-worker.md) |
| `eliot-search-doc-worker` | composition | binary | W10 | ≤5,000 | `search-contracts`, `search-provider-protocol`, `search-materializer` | [`eliot-search-doc-worker.md`](../../swarm/assignments/eliot-search-doc-worker.md) |

## Dependency rule

Capability/orchestration packages consume vendor-neutral ports from
[`PORT_CATALOG.md`](PORT_CATALOG.md). Concrete redb, OS-secret, Qdrant process and Qdrant data-plane
adapters are constructed only by `eliot-searchd`. A dependency does not transfer state ownership.

## Package-count rule

- Every package represents a real dependency, replacement, security, runtime, test or context boundary.
- Family directories never gain a forwarding crate.
- A package is not split merely to hold one type or one function.
- New packages require synchronized Cargo, registry, matrix, assignment, port/primitive ownership and launch-topology updates.

## Composition exception

`eliot-searchd` has the final dependency graph but composes it progressively through Cargo features.
Its W1 writer reads only W1 handoffs. Later layers consume accepted package/port receipts rather than
the whole architecture or implementation internals.
