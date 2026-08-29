# P00 contract implementation pack

This directory is the compact implementation projection for W0. It is derived from the embedded
Architecture 8.4 body with SHA-256
`ae4c18ccff256ce4d5fdf91dfd9041236ff6f332b611bae3bd748c2da8ac6a1c`.

It is not a second product architecture. On contradiction:

1. Part I of `ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md` wins;
2. stop the affected package with `CONTRACT_CHALLENGE`;
3. submit a contract-change request;
4. update this projection and its digest/receipt through the integration owner.

## Read sets

### `search-contracts` writer

Read all files in this directory. Implement shared strong types, tagged schemas, validation and
canonicalization. Do not implement I/O or port traits.

### `search-domain` writer

Read `CANONICAL_TYPES.md`, `SOURCE_GRAPH.md`, `QUERY_AND_RESULTS.md` and `REASON_CODES.md`, then consume
only the accepted `search-contracts` API digest.

### `search-ports` writer

Read `CANONICAL_TYPES.md`, `PORT_OPERATIONS.md`, `REASON_CODES.md` and the accepted
`search-contracts` API digest. Do not depend on `search-domain` or concrete adapters.

## Files

- `CANONICAL_TYPES.md` — primitive encodings, digest domains and strong-ID rules.
- `CONTRACT_CHALLENGES.md` — resolved handoff ambiguities and explicit precedence.
- `SOURCE_GRAPH.md` — source, ownership, residency, view and preparation schemas.
- `RECIPES.md` — exact v1 recipe IDs and typed request/output families.
- `QUERY_AND_RESULTS.md` — grants, budgets, plans, candidates, exact reports and coverage.
- `PROTOCOL_AND_LIFECYCLE.md` — provider envelope, capabilities, handles, security and lifecycle records.
- `REASON_CODES.md` — public, protocol, contract and internal error namespaces.
- `PORT_OPERATIONS.md` — shared vendor-neutral port operation inventory.

## Freeze receipt

The W0 receipt records:

- architecture hash;
- hashes of every file in this directory;
- `search-contracts`, `search-domain` and `search-ports` public API digests;
- exact toolchain/dependency/lockfile identity;
- contract/property/compile-fail test results;
- unresolved contract challenges, which must be empty before W1.
