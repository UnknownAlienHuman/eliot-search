# Agent contract — eliot-searchd

Own only `bins/eliot-searchd/`. This is the composition root, not a capability implementation package.
Do not edit library packages, root workspace, shared fixtures or architecture.

The bounded packet is `swarm/assignments/eliot-searchd.md`.

## Ownership

The #266 exception in `docs/audit/WAVE2_PACKAGE_EXCEPTION_MATRIX_2026-10-09.md` permits the
single manager to change only the named native/root/credential opening adapters and actual command
wiring. Existing-only inspection/opening and child shutdown precede durable owner release. This
does not authorize source/query/Qdrant or cryptographic implementation redesign.

- progressive dependency injection/startup
- concrete adapter construction and vendor-neutral port wiring
- provider server, readiness, drain and shutdown coordination
- qualified platform observations and stable integration reason mapping

## Forbidden ownership

- capability logic in `main`
- shared store/vendor clients outside daemon composition
- concrete adapter edges in query/lifecycle APIs
- hidden fallback or second data-root owner
- control-migration schemas, immutable artifact lifecycle or receipt projections
- revision-object read/write/publication state machines owned by `search-revision-store`
- preparation/materialization storage state machines owned by `search-materializer`
- secret protection semantics owned by `search-os-secrets`

## Dependencies

Only accepted packages for the active Cargo feature/wave. Baseline DIRECT owners such as source admission/registry/identity, safe reader, revision store and materializer are mandatory architecture boundaries, not optional acceptance claims. New artifacts require ADR and exact qualification.

## Size

Target `src/` ≤ 6,500 lines; split review before 8,500 total; hard stop at 10,000 hand-written Rust lines.
