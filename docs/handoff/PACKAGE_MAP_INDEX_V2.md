# Bounded package map index v2

This index is the Swarm entry point after a package is assigned. A package writer reads only:

1. its assignment and issued context bundle;
2. `overview.toml`;
3. the package-local operation, documentation and relation maps linked by the overview;
4. exact accepted dependency handoffs named by the relation map.

The maps do not authorize implementation and do not replace an issued ticket or lease.

| Package | Wave | Modules | Operations | Doc nodes | Dependencies | Map |
|---|---:|---:|---:|---:|---:|---|
| `eliot-search` | 1 | 11 | 19 | 128 | 4 | [`overview`](/swarm/coverage/package-maps/eliot-search/overview.toml) |
| `eliot-search-doc-worker` | 10 | 12 | 14 | 102 | 4 | [`overview`](/swarm/coverage/package-maps/eliot-search-doc-worker/overview.toml) |
| `eliot-search-model-worker` | 10 | 11 | 13 | 95 | 4 | [`overview`](/swarm/coverage/package-maps/eliot-search-model-worker/overview.toml) |
| `eliot-searchd` | 1 | 14 | 64 | 449 | 38 | [`overview`](/swarm/coverage/package-maps/eliot-searchd/overview.toml) |
| `search-access` | 4 | 10 | 13 | 117 | 3 | [`overview`](/swarm/coverage/package-maps/search-access/overview.toml) |
| `search-candidate-validator` | 4 | 9 | 7 | 70 | 4 | [`overview`](/swarm/coverage/package-maps/search-candidate-validator/overview.toml) |
| `search-code-enricher` | 5 | 11 | 9 | 95 | 3 | [`overview`](/swarm/coverage/package-maps/search-code-enricher/overview.toml) |
| `search-comparator` | 6 | 10 | 12 | 61 | 3 | [`overview`](/swarm/coverage/package-maps/search-comparator/overview.toml) |
| `search-config` | 1 | 9 | 10 | 90 | 1 | [`overview`](/swarm/coverage/package-maps/search-config/overview.toml) |
| `search-continuation` | 4 | 10 | 11 | 115 | 7 | [`overview`](/swarm/coverage/package-maps/search-continuation/overview.toml) |
| `search-contracts` | 0 | 13 | 0 | 243 | 0 | [`overview`](/swarm/coverage/package-maps/search-contracts/overview.toml) |
| `search-control-redb` | 1 | 11 | 20 | 95 | 4 | [`overview`](/swarm/coverage/package-maps/search-control-redb/overview.toml) |
| `search-domain` | 0 | 11 | 0 | 88 | 1 | [`overview`](/swarm/coverage/package-maps/search-domain/overview.toml) |
| `search-eliot-adapter` | 8 | 10 | 8 | 99 | 3 | [`overview`](/swarm/coverage/package-maps/search-eliot-adapter/overview.toml) |
| `search-epoch-pins` | 3 | 9 | 12 | 102 | 3 | [`overview`](/swarm/coverage/package-maps/search-epoch-pins/overview.toml) |
| `search-eval` | 4 | 15 | 36 | 285 | 3 | [`overview`](/swarm/coverage/package-maps/search-eval/overview.toml) |
| `search-exact` | 6 | 10 | 12 | 61 | 4 | [`overview`](/swarm/coverage/package-maps/search-exact/overview.toml) |
| `search-handles` | 4 | 11 | 10 | 115 | 4 | [`overview`](/swarm/coverage/package-maps/search-handles/overview.toml) |
| `search-index-reclaimer` | 3 | 9 | 11 | 125 | 5 | [`overview`](/swarm/coverage/package-maps/search-index-reclaimer/overview.toml) |
| `search-lexical` | 3 | 9 | 13 | 70 | 4 | [`overview`](/swarm/coverage/package-maps/search-lexical/overview.toml) |
| `search-materializer` | 2 | 11 | 18 | 76 | 3 | [`overview`](/swarm/coverage/package-maps/search-materializer/overview.toml) |
| `search-model-provider` | 10 | 9 | 21 | 108 | 3 | [`overview`](/swarm/coverage/package-maps/search-model-provider/overview.toml) |
| `search-os-secrets` | 1 | 10 | 16 | 65 | 4 | [`overview`](/swarm/coverage/package-maps/search-os-secrets/overview.toml) |
| `search-overlay` | 5 | 10 | 16 | 100 | 6 | [`overview`](/swarm/coverage/package-maps/search-overlay/overview.toml) |
| `search-point-identity` | 3 | 9 | 6 | 43 | 2 | [`overview`](/swarm/coverage/package-maps/search-point-identity/overview.toml) |
| `search-ports` | 0 | 14 | 0 | 73 | 1 | [`overview`](/swarm/coverage/package-maps/search-ports/overview.toml) |
| `search-projection-planner` | 3 | 10 | 7 | 52 | 4 | [`overview`](/swarm/coverage/package-maps/search-projection-planner/overview.toml) |
| `search-provider-protocol` | 1 | 14 | 20 | 154 | 4 | [`overview`](/swarm/coverage/package-maps/search-provider-protocol/overview.toml) |
| `search-publication` | 3 | 12 | 21 | 142 | 5 | [`overview`](/swarm/coverage/package-maps/search-publication/overview.toml) |
| `search-qdrant-bridge` | 3 | 10 | 16 | 149 | 4 | [`overview`](/swarm/coverage/package-maps/search-qdrant-bridge/overview.toml) |
| `search-qdrant-supervisor` | 3 | 10 | 9 | 67 | 4 | [`overview`](/swarm/coverage/package-maps/search-qdrant-supervisor/overview.toml) |
| `search-query-planner` | 4 | 11 | 7 | 75 | 5 | [`overview`](/swarm/coverage/package-maps/search-query-planner/overview.toml) |
| `search-research-export-adapter` | 8 | 10 | 10 | 99 | 4 | [`overview`](/swarm/coverage/package-maps/search-research-export-adapter/overview.toml) |
| `search-result-projector` | 4 | 9 | 6 | 53 | 5 | [`overview`](/swarm/coverage/package-maps/search-result-projector/overview.toml) |
| `search-retention` | 7 | 11 | 31 | 98 | 7 | [`overview`](/swarm/coverage/package-maps/search-retention/overview.toml) |
| `search-retrieval-executor` | 4 | 11 | 9 | 74 | 8 | [`overview`](/swarm/coverage/package-maps/search-retrieval-executor/overview.toml) |
| `search-revision-store` | 2 | 13 | 22 | 107 | 4 | [`overview`](/swarm/coverage/package-maps/search-revision-store/overview.toml) |
| `search-runtime-owner` | 1 | 10 | 17 | 68 | 4 | [`overview`](/swarm/coverage/package-maps/search-runtime-owner/overview.toml) |
| `search-safe-reader` | 2 | 10 | 19 | 77 | 4 | [`overview`](/swarm/coverage/package-maps/search-safe-reader/overview.toml) |
| `search-source-admission` | 2 | 9 | 16 | 72 | 4 | [`overview`](/swarm/coverage/package-maps/search-source-admission/overview.toml) |
| `search-source-identity` | 2 | 10 | 15 | 81 | 2 | [`overview`](/swarm/coverage/package-maps/search-source-identity/overview.toml) |
| `search-source-reconcile` | 5 | 10 | 18 | 106 | 7 | [`overview`](/swarm/coverage/package-maps/search-source-reconcile/overview.toml) |
| `search-source-registry` | 2 | 11 | 21 | 81 | 5 | [`overview`](/swarm/coverage/package-maps/search-source-registry/overview.toml) |
| `search-subject-resolver` | 6 | 9 | 12 | 66 | 2 | [`overview`](/swarm/coverage/package-maps/search-subject-resolver/overview.toml) |
| `search-unitizer` | 2 | 11 | 17 | 68 | 3 | [`overview`](/swarm/coverage/package-maps/search-unitizer/overview.toml) |

## Global reverse indexes

- `swarm/coverage/package-map-index.toml` — package-to-map index with exact digests.
- `swarm/coverage/documentation-file-index-v2.toml` — documentation file to package/module reverse index.
- `swarm/coverage/integration-map-v2.toml` — governance/navigation nodes explicitly outside product crates.
- `swarm/coverage/operation-modules.toml` — operation to exact package-local module.
- `swarm/coverage/dependency-edges.toml` — typed package/module dependency edges.
- `swarm/coverage/module-coverage.toml` — reverse relation counts and structural roles.
