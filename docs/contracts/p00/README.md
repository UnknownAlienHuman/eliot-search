# P00 contract implementation pack

This directory is the bounded W0 implementation projection derived from the embedded Architecture 8.4
body with SHA-256
`ae4c18ccff256ce4d5fdf91dfd9041236ff6f332b611bae3bd748c2da8ac6a1c`.

It is not a second product architecture. On contradiction:

1. Part I of `ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md` wins;
2. stop the affected package with `CONTRACT_CHALLENGE`;
3. submit a contract-change request;
4. update this projection and its hash receipt through the integration owner.

## Read sets

### `search-contracts`

Read every file in this directory. Implement shared strong types, tagged schemas, validation and
canonicalization. Do not implement I/O or port traits.

### `search-domain`

Read `CANONICAL_TYPES.md`, `SOURCE_GRAPH.md`, `QUERY_AND_RESULTS.md`, `RECIPE_RESULTS.md` and
`REASON_CODES.md`, then consume only the accepted contracts API digest.

### `search-ports`

Read `CANONICAL_TYPES.md`, `PORT_OPERATIONS.md`, `REASON_CODES.md` and the accepted contracts API
digest. Do not depend on domain or adapters.

## Files

- `CANONICAL_TYPES.md` — primitive encodings, digest domains and strong-ID rules.
- `CONTRACT_CHALLENGES.md` — resolved handoff ambiguities and precedence.
- `SOURCE_GRAPH.md` — source, ownership, residency, view and preparation schemas.
- `RECIPES.md` — exact v1 recipe IDs and typed request bodies.
- `QUERY_AND_RESULTS.md` — grants, budgets, plans, validated candidates, gaps and exact reports.
- `RECIPE_RESULTS.md` — field-level outputs and exact tagged result union for all eleven recipes.
- `PROTOCOL_AND_LIFECYCLE.md` — provider envelope, capabilities, handles, security and lifecycle.
- `REASON_CODES.md` — public, protocol, contract and internal error namespaces.
- `PORT_OPERATIONS.md` — shared vendor-neutral port operation inventory.

## Freeze receipt

W0 records architecture and file hashes, contracts/domain/ports API digests, exact toolchain/dependency
identity, contract/property/compile-fail outcomes and an empty unresolved-challenge set.
