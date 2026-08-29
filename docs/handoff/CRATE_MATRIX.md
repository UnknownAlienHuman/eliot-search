# Crate ownership matrix

This is the human ownership/index view. `swarm/crates.toml` alone owns exact dependency lists,
package paths, optionality, assignments, function packets, configuration sections and qualification
packets; this file intentionally does not duplicate those edges.

- **Library packages:** 41
- **Binary packages:** 4
- **Total one-writer packages:** 45
- **Normal `src/` target:** at most 7,500 hand-written lines
- **Split review:** before 8,500 total hand-written lines
- **Hard stop:** 10,000 including package-local tests

## Foundation

| Package | Boundary | Earliest wave | Target | Assignment |
|---|---|---:|---:|---|
| `search-contracts` | C00 serialized records, IDs and reason registries | W0 | 7,500 | [`search-contracts.md`](../../swarm/assignments/search-contracts.md) |
| `search-domain` | pure transition/eligibility/order/coverage kernel | W0 | 7,000 | [`search-domain.md`](../../swarm/assignments/search-domain.md) |
| `search-ports` | shared vendor-neutral traits and conformance fakes | W0 | 5,500 | [`search-ports.md`](../../swarm/assignments/search-ports.md) |
| `search-config` | pure configuration layering, redaction and reconfiguration planning | W1 | 5,500 | [`search-config.md`](../../swarm/assignments/search-config.md) |

## Runtime and control

| Package | Boundary | Earliest wave | Target | Assignment |
|---|---|---:|---:|---|
| `search-runtime-owner` | C01 data-root owner epoch/lease | W1 | 4,500 | [`search-runtime-owner.md`](../../swarm/assignments/search-runtime-owner.md) |
| `search-os-secrets` | OS-bound opaque secret lifecycle | W1 | 3,500 | [`search-os-secrets.md`](../../swarm/assignments/search-os-secrets.md) |
| `search-control-redb` | C02 bounded control journal | W1 | 7,500 | [`search-control-redb.md`](../../swarm/assignments/search-control-redb.md) |
| `search-retention` | C28 CAS retention, purge and restore | W7 | 7,500 | [`search-retention.md`](../../swarm/assignments/search-retention.md) |

## Source

| Package | Boundary | Earliest wave | Target | Assignment |
|---|---|---:|---:|---|
| `search-source-admission` | deny-by-default admission evaluator | W2 | 4,500 | [`search-source-admission.md`](../../swarm/assignments/search-source-admission.md) |
| `search-source-registry` | C03 roots, memberships, portfolios, views and cutover | W2 | 6,500 | [`search-source-registry.md`](../../swarm/assignments/search-source-registry.md) |
| `search-source-identity` | C04 source identity and path history | W2 | 6,500 | [`search-source-identity.md`](../../swarm/assignments/search-source-identity.md) |
| `search-source-reconcile` | C05 observation and reconciliation | W5 | 7,000 | [`search-source-reconcile.md`](../../swarm/assignments/search-source-reconcile.md) |
| `search-safe-reader` | C06 stable no-execute acquisition | W2 | 6,500 | [`search-safe-reader.md`](../../swarm/assignments/search-safe-reader.md) |
| `search-revision-store` | C07 residency-aware immutable CAS/readback | W2 | 7,500 | [`search-revision-store.md`](../../swarm/assignments/search-revision-store.md) |

## Preparation and providers

| Package | Boundary | Earliest wave | Target | Assignment |
|---|---|---:|---:|---|
| `search-materializer` | C08 canonical materialization/loss maps | W2 | 7,000 | [`search-materializer.md`](../../swarm/assignments/search-materializer.md) |
| `search-unitizer` | C09 deterministic unit occurrences | W2 | 6,500 | [`search-unitizer.md`](../../swarm/assignments/search-unitizer.md) |
| `search-code-enricher` | C10 structural facts and assurance | W5 | 7,500 | [`search-code-enricher.md`](../../swarm/assignments/search-code-enricher.md) |
| `search-lexical` | C11 deterministic sparse encoding | W3 | 7,500 | [`search-lexical.md`](../../swarm/assignments/search-lexical.md) |
| `search-model-provider` | C12 optional semantic/rerank provider | W10 | 6,500 | [`search-model-provider.md`](../../swarm/assignments/search-model-provider.md) |

## Index

| Package | Boundary | Earliest wave | Target | Assignment |
|---|---|---:|---:|---|
| `search-projection-planner` | C13 exact point/manifests planning | W3 | 7,000 | [`search-projection-planner.md`](../../swarm/assignments/search-projection-planner.md) |
| `search-point-identity` | C14 canonical key and collision guard | W3 | 4,500 | [`search-point-identity.md`](../../swarm/assignments/search-point-identity.md) |
| `search-qdrant-supervisor` | exact Qdrant process/artifact containment | W3 | 5,500 | [`search-qdrant-supervisor.md`](../../swarm/assignments/search-qdrant-supervisor.md) |
| `search-qdrant-bridge` | C15 Qdrant data-plane adapter | W3 | 7,000 | [`search-qdrant-bridge.md`](../../swarm/assignments/search-qdrant-bridge.md) |
| `search-publication` | C16 linearizable epoch publication | W3 | 7,500 | [`search-publication.md`](../../swarm/assignments/search-publication.md) |
| `search-epoch-pins` | C17 route/epoch pin registry | W3 | 4,500 | [`search-epoch-pins.md`](../../swarm/assignments/search-epoch-pins.md) |
| `search-index-reclaimer` | exact ordinary retired-point deletion | W3 | 4,500 | [`search-index-reclaimer.md`](../../swarm/assignments/search-index-reclaimer.md) |

## Query

| Package | Boundary | Earliest wave | Target | Assignment |
|---|---|---:|---:|---|
| `search-access` | C18 grants, pre-candidate filters and live deny | W4 | 7,500 | [`search-access.md`](../../swarm/assignments/search-access.md) |
| `search-overlay` | C19 saved/unsaved transient overlay | W5 | 7,500 | [`search-overlay.md`](../../swarm/assignments/search-overlay.md) |
| `search-exact` | C20 frozen-denominator exact proof | W6 | 7,500 | [`search-exact.md`](../../swarm/assignments/search-exact.md) |
| `search-subject-resolver` | C21 ambiguity-preserving resolution | W6 | 6,000 | [`search-subject-resolver.md`](../../swarm/assignments/search-subject-resolver.md) |
| `search-query-planner` | C22 server-owned bounded plan | W4 | 7,500 | [`search-query-planner.md`](../../swarm/assignments/search-query-planner.md) |
| `search-retrieval-executor` | C23 bounded leg execution/fusion | W4 | 7,500 | [`search-retrieval-executor.md`](../../swarm/assignments/search-retrieval-executor.md) |
| `search-candidate-validator` | C24 exact source-backed validation | W4 | 7,500 | [`search-candidate-validator.md`](../../swarm/assignments/search-candidate-validator.md) |
| `search-comparator` | C25 descriptive behavior comparison | W6 | 7,500 | [`search-comparator.md`](../../swarm/assignments/search-comparator.md) |
| `search-handles` | source-handle state and expansion authorization | W4 | 6,500 | [`search-handles.md`](../../swarm/assignments/search-handles.md) |
| `search-result-projector` | C26 bounded result cards | W4 | 7,000 | [`search-result-projector.md`](../../swarm/assignments/search-result-projector.md) |
| `search-continuation` | C27 bounded continuation state | W4 | 6,000 | [`search-continuation.md`](../../swarm/assignments/search-continuation.md) |

## Evaluation and edges

| Package | Boundary | Earliest wave | Target | Assignment |
|---|---|---:|---:|---|
| `search-eval` | C29 control corpus and Product Pulse | W4 | 7,500 | [`search-eval.md`](../../swarm/assignments/search-eval.md) |
| `search-provider-protocol` | C30 generic frame/session/binding edge | W1 | 7,500 | [`search-provider-protocol.md`](../../swarm/assignments/search-provider-protocol.md) |
| `search-eliot-adapter` | optional ELIOT leaf mapping | W8 | 5,500 | [`search-eliot-adapter.md`](../../swarm/assignments/search-eliot-adapter.md) |
| `search-research-export-adapter` | optional Research export leaf | W8 | 6,000 | [`search-research-export-adapter.md`](../../swarm/assignments/search-research-export-adapter.md) |

## Binaries

| Package | Boundary | Earliest wave | Target | Assignment |
|---|---|---:|---:|---|
| `eliot-searchd` | progressive composition root | W1 | 6,500 | [`eliot-searchd.md`](../../swarm/assignments/eliot-searchd.md) |
| `eliot-search` | standalone daemon client CLI | W1 | 4,500 | [`eliot-search.md`](../../swarm/assignments/eliot-search.md) |
| `eliot-search-model-worker` | optional model worker | W10 | 4,500 | [`eliot-search-model-worker.md`](../../swarm/assignments/eliot-search-model-worker.md) |
| `eliot-search-doc-worker` | optional document worker | W10 | 5,000 | [`eliot-search-doc-worker.md`](../../swarm/assignments/eliot-search-doc-worker.md) |

## Registry extensions

`swarm/crates.toml` may additionally bind a package to:

- a package-local `FUNCTIONS.md` operation contract;
- one or more capability-owned configuration packets under `config/sections/`;
- an external-artifact qualification packet such as `qualification/qdrant/W3_QUALIFICATION.md`.

Those paths are part of the bounded writer read set but do not authorize implementation. Current
authorization remains solely in `swarm/launch-state.toml`.

## Package-count and dependency rule

Every package must represent a real dependency, replacement, security, runtime, test or context
boundary. Family directories never gain forwarding crates. Adding a package requires one atomic
integration change covering Cargo, registry, assignment, ownership, fixtures, configuration ownership
and launch topology.
