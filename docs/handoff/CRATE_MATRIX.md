# Crate matrix

This is the implementation ownership map for the one-agent/one-package swarm. A row is a write
boundary, not merely a namespace. Family directories have no `Cargo.toml` and own no behavior.

- **Library packages:** 39
- **Binary packages:** 4
- **Total Cargo packages:** 43
- **Hard review threshold:** 10,000 hand-written Rust lines per package
- **Architecture master:** 8.4
- **Embedded architecture SHA-256:** `ae4c18ccff256ce4d5fdf91dfd9041236ff6f332b611bae3bd748c2da8ac6a1c`

The support packages introduced by ADR 0002 do not add new product authority. They isolate
security/runtime owners that were either unowned or hidden inside a larger capability cell.

| Package | Kind | Cell / support boundary | Family | Delivery | Soft lines | Optional | Direct package dependencies |
|---|---|---|---|---|---:|---|---|
| `search-contracts` | lib | C00 | foundation | W0 / P00 | 8,000 | no | — |
| `search-domain` | lib | shared pure kernel | foundation | W0 / P00 | 7,000 | no | `search-contracts` |
| `search-runtime-owner` | lib | C01 | runtime | W1 / P01 | 4,500 | no | `search-contracts`, `search-domain` |
| `search-os-secrets` | lib | C01/C15/C30 security support | runtime | W1 / P01 foundation; qualified with P05/P14 | 3,500 | no | `search-contracts`, `search-domain` |
| `search-control-redb` | lib | C02 | control | W1 / P02 | 8,500 | no | `search-contracts`, `search-domain` |
| `search-source-admission` | lib | C03/C06 security support | source | W2 / P03 | 4,500 | no | `search-contracts`, `search-domain` |
| `search-source-registry` | lib | C03 | source | W2 / P03 | 6,500 | no | `search-contracts`, `search-domain`, `search-source-identity`, `search-source-admission` |
| `search-source-identity` | lib | C04 | source | W2 / P03 | 6,500 | no | `search-contracts`, `search-domain` |
| `search-source-reconcile` | lib | C05 | source | W5 / P09 | 7,000 | no | `search-contracts`, `search-domain`, `search-source-registry`, `search-source-identity`, `search-safe-reader` |
| `search-safe-reader` | lib | C06 | source | W2 / P03 | 6,500 | no | `search-contracts`, `search-domain` |
| `search-revision-store` | lib | C07 | source | W2 / P04 | 8,000 | no | `search-contracts`, `search-domain` |
| `search-materializer` | lib | C08 | preparation | W2 baseline / P04; optional P17 | 7,000 | no | `search-contracts`, `search-domain` |
| `search-unitizer` | lib | C09 | preparation | W2 / P04-P06 | 6,500 | no | `search-contracts`, `search-domain` |
| `search-code-enricher` | lib | C10 | preparation | W5 / P10 | 8,500 | no | `search-contracts`, `search-domain` |
| `search-lexical` | lib | C11 | provider | W3 / P06 | 8,500 | no | `search-contracts`, `search-domain` |
| `search-model-provider` | lib | C12 | provider | W10 / P16 after accepted P15 | 6,500 | yes | `search-contracts`, `search-domain` |
| `search-projection-planner` | lib | C13 | index | W3 / P06 | 7,000 | no | `search-contracts`, `search-domain`, `search-point-identity` |
| `search-point-identity` | lib | C14 | index | W3 / P06 | 4,500 | no | `search-contracts`, `search-domain` |
| `search-qdrant-supervisor` | lib | C01/C15 process support | index | W3 / P05 | 5,500 | no | `search-contracts`, `search-domain`, `search-os-secrets` |
| `search-qdrant-bridge` | lib | C15 data plane | index | W3 / P05 | 7,000 | no | `search-contracts`, `search-domain` |
| `search-publication` | lib | C16 | index | W3 / P07 | 9,500 | no | `search-contracts`, `search-domain`, `search-projection-planner`, `search-point-identity`, `search-epoch-pins` |
| `search-epoch-pins` | lib | C17 pin registry | index | W3 / P07 | 4,500 | no | `search-contracts`, `search-domain` |
| `search-index-reclaimer` | lib | C17 reclaim executor | index | W3 / P07 | 4,500 | no | `search-contracts`, `search-domain`, `search-epoch-pins` |
| `search-access` | lib | C18 | query | W4 / P08; hardened P13 | 8,500 | no | `search-contracts`, `search-domain` |
| `search-overlay` | lib | C19 | query | W5 / P09 | 8,000 | no | `search-contracts`, `search-domain`, `search-unitizer`, `search-lexical` |
| `search-exact` | lib | C20 | query | W6 / P12 | 8,500 | no | `search-contracts`, `search-domain`, `search-access` |
| `search-subject-resolver` | lib | C21 | query | W6 / P11 | 6,000 | no | `search-contracts`, `search-domain` |
| `search-query-planner` | lib | C22 | query | W4 / P08 | 9,000 | no | `search-contracts`, `search-domain`, `search-access` |
| `search-retrieval-executor` | lib | C23 | query | W4 / P08 | 9,500 | no | `search-contracts`, `search-domain`, `search-query-planner`, `search-lexical`, `search-epoch-pins`, `search-access` |
| `search-candidate-validator` | lib | C24 | query | W4 / P08; hardened P13 | 8,000 | no | `search-contracts`, `search-domain`, `search-access` |
| `search-comparator` | lib | C25 | query | W6 / P11 | 8,000 | no | `search-contracts`, `search-domain`, `search-subject-resolver` |
| `search-handles` | lib | C26/C27 handle support | query | W4 / P08; hardened P13 | 6,500 | no | `search-contracts`, `search-domain` |
| `search-result-projector` | lib | C26 | query | W4 / P08 | 7,000 | no | `search-contracts`, `search-domain`, `search-candidate-validator`, `search-handles` |
| `search-continuation` | lib | C27 | query | W4 / P08; hardened P13 | 6,000 | no | `search-contracts`, `search-domain`, `search-query-planner`, `search-access`, `search-epoch-pins` |
| `search-retention` | lib | C28 | runtime | W7 / P13 | 8,500 | no | `search-contracts`, `search-domain`, `search-epoch-pins`, `search-index-reclaimer`, `search-handles` |
| `search-eval` | lib | C29 | evaluation | W4 baseline / P08; acceptance W9 / P15 | 8,500 | no | `search-contracts`, `search-domain` |
| `search-provider-protocol` | lib | C30 generic edge | adapter | W1 / P01 transport; W8 / P14 integration | 8,500 | no | `search-contracts`, `search-domain` |
| `search-eliot-adapter` | lib | C30 optional ELIOT profile | adapter | W8 / optional P14 profile | 5,500 | yes | `search-contracts`, `search-domain`, `search-provider-protocol` |
| `search-research-export-adapter` | lib | C30 optional Research profile | adapter | W8 / optional P14 profile | 6,000 | yes | `search-contracts`, `search-domain`, `search-provider-protocol` |
| `eliot-searchd` | bin | composition | binary | W1 shell, integrated through W9 | 6,500 | no | `search-contracts`, `search-domain`, `search-runtime-owner`, `search-os-secrets`, `search-control-redb`, `search-source-admission`, `search-source-registry`, `search-source-identity`, `search-source-reconcile`, `search-safe-reader`, `search-revision-store`, `search-materializer`, `search-unitizer`, `search-code-enricher`, `search-lexical`, `search-projection-planner`, `search-point-identity`, `search-qdrant-supervisor`, `search-qdrant-bridge`, `search-publication`, `search-epoch-pins`, `search-index-reclaimer`, `search-access`, `search-overlay`, `search-exact`, `search-subject-resolver`, `search-query-planner`, `search-retrieval-executor`, `search-candidate-validator`, `search-comparator`, `search-handles`, `search-result-projector`, `search-continuation`, `search-retention`, `search-eval`, `search-provider-protocol` |
| `eliot-search` | bin | composition | binary | W1 shell, commands added by owning packages | 4,500 | no | `search-contracts`, `search-provider-protocol` |
| `eliot-search-model-worker` | bin | composition | binary | W10 / P16 after accepted P15 | 4,500 | yes | `search-contracts`, `search-provider-protocol`, `search-model-provider` |
| `eliot-search-doc-worker` | bin | composition | binary | W10 / P17 after accepted P15 | 5,000 | yes | `search-contracts`, `search-provider-protocol`, `search-materializer` |

## Dependency rule

Core/query/lifecycle packages consume vendor-neutral ports from `search-contracts` / `search-domain`.
Concrete redb, Qdrant process and Qdrant transport packages are wired only by `eliot-searchd`.
A dependency does not transfer ownership: consumers may call an accepted public port but may not
edit or reinterpret the producer's state.

## Ownership rules

The package's `AGENTS.md` is the complete ordinary implementation brief: mission, owned state,
forbidden state, logical surface, reason codes, tests and line budget. The machine-readable equivalent
is [`../../swarm/crates.toml`](../../swarm/crates.toml). Missing contracts use the contract-change path
in [`SWARM_PROTOCOL.md`](SWARM_PROTOCOL.md).
