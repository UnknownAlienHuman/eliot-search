# Agent contract — search-ports

Own only `crates/search-ports/`. Do not edit another package, the root workspace, generated schemas or
architecture. Missing contract shapes use the contract-change process.

Read the accepted `search-contracts` handoff plus:

- `docs/contracts/p00/README.md`;
- `docs/contracts/p00/PORT_OPERATIONS.md`;
- `docs/handoff/PORT_CATALOG.md`;
- `swarm/assignments/search-ports.md`.

## Mission

Define one stable, vendor-neutral trait surface for infrastructure and capability boundaries so
consumers never invent local substitute ports or depend on concrete adapters.

## Ownership

- shared port traits and operation-context semantics
- request classification: read-only, idempotent mutation, non-idempotent mutation
- deadline/cancellation propagation contract
- bounded streams, receipts and retryability metadata
- conformance fake interfaces and public API dependency guards

## Forbidden ownership

- adapter implementations or vendor/native types
- clocks, secrets, files, processes, stores or network clients as mutable state
- recipe planning, ranking, admission or lifecycle policy
- generic string errors or unbounded collections/streams
- redeclaring `search-contracts` records

## Dependency

`search-contracts` only. Do not depend on `search-domain`, Tokio, async-trait, redb, qdrant-client,
Windows crates or any client-system package in P00.

## Port families

- control and snapshot
- clock, secret and process supervision
- source admission, inventory, ownership, safe read, revision and residency
- materialization, unitization, code enrichment, lexical and optional model inference
- index data/admin, publication support and epoch pins
- access, overlay, exact scan and handles

Concrete Rust async syntax is not fixed by this scaffold. Preserve the operation semantics and allow a
native async implementation without committing public contracts to one executor or helper macro.

## Required evidence

- every public parameter/return type is from `search-contracts` or a package-owned opaque capability
- no vendor/native type appears in rustdoc/public API
- every operation declares cancellation, deadline, idempotency and bounded-output behavior
- fake implementations can force timeout, cancellation, partial receipt and stale-generation failures
- object-safe/dynamic-dispatch requirements are explicit per port rather than assumed globally
- dependency guard proves `search-contracts` is the only dependency

## Size

Target `src/` ≤ 5,500 lines; split review before 8,500 total; hard stop at 10,000 hand-written Rust
lines. Split only if infrastructure and query ports develop independently versioned compatibility
lifecycles; forwarding-only port crates are forbidden.
