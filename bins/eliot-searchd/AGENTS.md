# Agent contract — eliot-searchd

You own only `bins/eliot-searchd/`. This is a composition package, not a place to reimplement
capability logic. Do not edit library packages, the root workspace, shared fixtures or architecture.
Traceability only: S0, S27, S29, S30.3, S32, S33, P01-P15.

## Mission

Compose the capability crates, own the data root and expose the only storage/index/provider process
boundary.

## Ownership

- dependency injection and startup order
- data-root owner guard
- composition of vendor-neutral ports with concrete redb/Qdrant/process/OS adapters
- bounded task supervision
- provider protocol server
- controlled shutdown and readiness/degradation reporting

## Forbidden ownership

- reimplementing capability logic inside `main`
- sharing store clients with CLI/workers/adapters
- allowing another process to own Qdrant or redb
- allowing query/lifecycle packages to construct concrete adapters
- hidden fallback across capability boundaries

## Allowed dependencies

`search-contracts`, `search-domain`, `search-runtime-owner`, `search-os-secrets`,
`search-control-redb`, `search-source-admission`, `search-source-registry`, `search-source-identity`,
`search-source-reconcile`, `search-safe-reader`, `search-revision-store`, `search-materializer`,
`search-unitizer`, `search-code-enricher`, `search-lexical`, `search-projection-planner`,
`search-point-identity`, `search-qdrant-supervisor`, `search-qdrant-bridge`, `search-publication`,
`search-epoch-pins`, `search-index-reclaimer`, `search-access`, `search-overlay`, `search-exact`,
`search-subject-resolver`, `search-query-planner`, `search-retrieval-executor`,
`search-candidate-validator`, `search-comparator`, `search-handles`, `search-result-projector`,
`search-continuation`, `search-retention`, `search-eval`, `search-provider-protocol`.

Do not add a storage, index, model, parser or client implementation outside this declared graph.
External artifacts require an ADR, exact version/digest, license proof and qualification.

## Required composition order

1. acquire `search-runtime-owner`;
2. open `search-os-secrets` and `search-control-redb`;
3. publish immutable control/security snapshots;
4. compose source admission/identity/registry/readback/revision ports;
5. when enabled, start `search-qdrant-supervisor`, then connect `search-qdrant-bridge`;
6. compose publication, pins and `search-index-reclaimer`;
7. compose query, `search-handles`, continuation and retention ports;
8. expose `search-provider-protocol`.

No capability crate receives concrete credentials or a raw vendor client.

## Integration milestones

- P01: owner/secrets/transport shell and clean shutdown
- P02-P04: direct source profile composition
- P05-P08: qualified Qdrant process/data plane, lexical index, reclaimer, query and handles
- P09-P13: reconciliation, code, exact, revocation, retention and purge hardening
- P14-P15: generic client edge and product qualification

## Test seams and exit evidence

- `CLI workers and adapters cannot reach stores directly`
- `startup orders owner→secrets→journal→source→optional Qdrant supervisor→bridge`
- `query packages receive ports not Qdrant clients`
- `degraded direct mode is truthful`
- `shutdown cancels work releases pins expires handles and terminates Job Object`
- `second daemon cannot own same root`
- `dependency graph contains no reverse adapter edge`

## Size and split guard

- Delivery wave: **W1 shell, integrated through W9**
- Soft `src/` target: **6,500 lines**
- Hard review threshold: **10,000 hand-written Rust lines**
- Keep `main` and command wiring thin; behavior belongs to the owning library package.

## Definition of done

The binary only composes accepted package contracts, has no reverse storage/authority path, enforces
bounded lifecycle/cancellation and supplies a reproducible handoff. Compilation alone is insufficient.
