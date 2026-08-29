# Agent contract — eliot-search

You own only `bins/eliot-search/`. This is a composition or worker package, not a place to reimplement
capability logic. Do not edit library packages, the root workspace, shared fixtures or architecture.
Traceability only: S1.2, S33, H11.6, P01, P07, P14.

## Mission

Expose standalone commands strictly through the generic provider protocol.

## Ownership

- argument parsing
- local binding/bootstrap UX
- request construction
- bounded result rendering
- doctor command transport

## Forbidden ownership

- opening redb, CAS or Qdrant
- reimplementing query/access logic
- minting unbounded grants
- rendering hidden raw payloads

## Allowed dependencies

`search-contracts`, `search-provider-protocol`. Do not add a storage, index, model, parser or client implementation outside this declared
graph. New external artifacts require an ADR, exact version/digest, license proof and qualification.

## Integration milestones

- P01: daemon connection and health
- P03-P12: thin recipe/doctor commands
- P14: final protocol negotiation and flow control

## Test seams and exit evidence

- `binary dependency graph has no store/index crates`
- `all commands use ProviderEnvelope`
- `result rendering obeys disclosure/byte limits`
- `disconnect/cancel is idempotent`

Record exact command output and degraded behavior. Do not claim a Windows, Qdrant or provider proof that
was not executed.

## Size and split guard

- Delivery wave: **W1 shell, commands added by owning packages**
- Soft `src/` target: **4,500 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Keep `main` and command wiring thin; behavior belongs to the owning library package.

## Definition of done

The binary only composes accepted package contracts, has no reverse storage/authority path, enforces
bounded lifecycle and cancellation, and supplies a reproducible handoff using
`swarm/PACKAGE_HANDOFF_TEMPLATE.md`. Compilation alone is insufficient.
