# P00 contract implementation pack

This is the bounded W0 projection derived from Architecture 8.4 SHA-256
`ae4c18ccff256ce4d5fdf91dfd9041236ff6f332b611bae3bd748c2da8ac6a1c`.
It is not a second architecture. Part I wins; contradiction stops work and updates this pack through
the integration owner.

## Read sets

- `search-contracts`: every file in this directory.
- `search-domain`: canonical types, type registry, source graph, query/results, recipe results and
  reasons, then the accepted contracts digest.
- `search-ports`: canonical types, type registry, port operations, reasons and the accepted contracts
  digest.

## Files

- `CANONICAL_TYPES.md` — primitive encodings and digest domains.
- `TYPE_REGISTRY.md` — helper records, visibility, bounds, enums and port support types.
- `CONTRACT_CHALLENGES.md` — precedence and ambiguity decisions.
- `SOURCE_GRAPH.md` — source, ownership, residency, view and preparation schemas.
- `RECIPES.md` — exact request registry/bodies.
- `QUERY_AND_RESULTS.md` — grants, plans, validated candidates, gaps and exact reports.
- `RECIPE_RESULTS.md` — all eleven field-level result variants.
- `PROTOCOL_AND_LIFECYCLE.md` — protocol, opaque handles, server records and lifecycle.
- `REASON_CODES.md` — public/protocol/contract/local error namespaces.
- `PORT_OPERATIONS.md` — shared vendor-neutral operation inventory.

## Freeze receipt

W0 records architecture/manifest/file hashes, contracts/domain/ports API digests, exact
toolchain/dependency identity, test outcomes and zero unresolved challenges.
