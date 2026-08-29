# Agent contract — eliot-searchd

You own only `bins/eliot-searchd/`. This is a composition or worker package, not a place to reimplement
capability logic. Do not edit library packages, the root workspace, shared fixtures or architecture.
Traceability only: S0, S27, S29, S30.3, S32, S33, P01-P15.

## Mission

Compose the capability crates, own the data root and expose the only storage/provider process boundary.

## Ownership

- dependency wiring and startup order
- data-root owner guard
- bounded task supervision
- provider protocol server
- controlled shutdown and readiness/degradation reporting

## Forbidden ownership

- reimplementing capability logic inside main
- sharing store clients with CLI/workers/adapters
- allowing another process to own qdrant or redb
- hidden fallback across capability boundaries

## Allowed dependencies

`search-contracts`, `search-domain`, `search-runtime-owner`, `search-control-redb`, `search-source-registry`, `search-source-identity`, `search-source-reconcile`, `search-safe-reader`, `search-revision-store`, `search-materializer`, `search-unitizer`, `search-code-enricher`, `search-lexical`, `search-projection-planner`, `search-point-identity`, `search-qdrant-bridge`, `search-publication`, `search-epoch-pins`, `search-access`, `search-overlay`, `search-exact`, `search-subject-resolver`, `search-query-planner`, `search-retrieval-executor`, `search-candidate-validator`, `search-comparator`, `search-result-projector`, `search-continuation`, `search-retention`, `search-eval`, `search-provider-protocol`. Do not add a storage, index, model, parser or client implementation outside this declared
graph. New external artifacts require an ADR, exact version/digest, license proof and qualification.

## Integration milestones

- P01: owner shell, framing entry, clean shutdown
- P02-P04: direct profile composition
- P05-P08: lexical profile composition
- P09-P13: reconciliation, code, exact and purge hardening
- P14-P15: generic client edge and product qualification

## Test seams and exit evidence

- `CLI and workers cannot reach stores directly`
- `startup orders owner→journal→source→optional Qdrant`
- `degraded direct mode is truthful`
- `shutdown cancels work and releases pins`
- `second daemon cannot own same root`

Record exact command output and degraded behavior. Do not claim a Windows, Qdrant or provider proof that
was not executed.

## Size and split guard

- Delivery wave: **W1 shell, integrated through W9**
- Soft `src/` target: **6,500 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Keep `main` and command wiring thin; behavior belongs to the owning library package.

## Definition of done

The binary only composes accepted package contracts, has no reverse storage/authority path, enforces
bounded lifecycle and cancellation, and supplies a reproducible handoff using
`swarm/PACKAGE_HANDOFF_TEMPLATE.md`. Compilation alone is insufficient.
