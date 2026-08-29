# Agent contract — eliot-search-model-worker

You own only `bins/eliot-search-model-worker/`. This is a composition or worker package, not a place to reimplement
capability logic. Do not edit library packages, the root workspace, shared fixtures or architecture.
Traceability only: S29, P16.

## Mission

Host an admitted optional model provider in an isolated on-demand process.

## Ownership

- worker lifecycle and IPC
- resource limits
- model-provider request dispatch
- health/cancellation reporting

## Forbidden ownership

- starting before P15+ADR
- redb/Qdrant ownership or direct access
- canonical decisions
- persistent hidden model cache

## Allowed dependencies

`search-contracts`, `search-provider-protocol`, `search-model-provider`. Do not add a storage, index, model, parser or client implementation outside this declared
graph. New external artifacts require an ADR, exact version/digest, license proof and qualification.

## Integration milestones

- P16 only after explicit acceptance gate

## Test seams and exit evidence

- `feature absent by default`
- `worker removal restores baseline`
- `resource/cancel limits`
- `no control-store access`

Record exact command output and degraded behavior. Do not claim a Windows, Qdrant or provider proof that
was not executed.

## Size and split guard

- Delivery wave: **W10 / P16 after accepted P15**
- Soft `src/` target: **4,500 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Keep `main` and command wiring thin; behavior belongs to the owning library package.

## Gate

This binary is optional. Do not implement, package or enable it before the stated P15 acceptance gate and ADR.

## Definition of done

The binary only composes accepted package contracts, has no reverse storage/authority path, enforces
bounded lifecycle and cancellation, and supplies a reproducible handoff using
`swarm/PACKAGE_HANDOFF_TEMPLATE.md`. Compilation alone is insufficient.
