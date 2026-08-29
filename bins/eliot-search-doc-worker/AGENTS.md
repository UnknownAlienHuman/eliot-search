# Agent contract — eliot-search-doc-worker

You own only `bins/eliot-search-doc-worker/`. This is a composition or worker package, not a place to reimplement
capability logic. Do not edit library packages, the root workspace, shared fixtures or architecture.
Traceability only: S17, S29, P17.

## Mission

Host one ADR-qualified document materializer in an isolated no-execute process.

## Ownership

- worker lifecycle and IPC
- provider sandbox/resource limits
- materialization request dispatch
- crash/malformed-input isolation

## Forbidden ownership

- provider selection in scaffold
- redb/Qdrant ownership or direct access
- macros, remote resources or archive execution
- Python/Node runtime without explicit ADR

## Allowed dependencies

`search-contracts`, `search-provider-protocol`, `search-materializer`. Do not add a storage, index, model, parser or client implementation outside this declared
graph. New external artifacts require an ADR, exact version/digest, license proof and qualification.

## Integration milestones

- P17 only after explicit acceptance gate and provider ADR

## Test seams and exit evidence

- `feature absent by default`
- `malformed-input isolation`
- `provider removal test`
- `no store/index access`

Record exact command output and degraded behavior. Do not claim a Windows, Qdrant or provider proof that
was not executed.

## Size and split guard

- Delivery wave: **W10 / P17 after accepted P15**
- Soft `src/` target: **5,000 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Keep `main` and command wiring thin; behavior belongs to the owning library package.

## Gate

This binary is optional. Do not implement, package or enable it before the stated P15 acceptance gate and ADR.

## Definition of done

The binary only composes accepted package contracts, has no reverse storage/authority path, enforces
bounded lifecycle and cancellation, and supplies a reproducible handoff using
`swarm/PACKAGE_HANDOFF_TEMPLATE.md`. Compilation alone is insufficient.
