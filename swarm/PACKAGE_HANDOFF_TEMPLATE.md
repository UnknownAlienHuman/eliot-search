# Package handoff

## Identity

- Package / assignment / wave:
- Base and final commits:
- Agent / reviewer:
- Accepted dependency commits and API/port digests:

## Changed files and scope

List every changed file. Record any integration-owner exception; otherwise every path must remain under
the package directory.

## Public surface

- added/changed public records or package-owned opaque types;
- ports implemented and consumed;
- canonical public API/schema/port digest;
- operation inventory with idempotency, cancellation, deadline and bounds;
- proof that no vendor/native type or credential crosses the API.

## State ownership

- mutable state owned here;
- dependency-owned state referenced only opaquely;
- confirmation that no second store/catalog/handle/policy/deletion owner was introduced.

## Failure mapping

| Local typed error | Public reason | Protocol error | Retryability | Disclosure |
|---|---|---|---|---|

## Tests and raw outcomes

Record exact commands and raw pass/fail/skip summaries, platform, toolchain and external artifact
versions/digests. Never infer a pass for an unavailable command.

## Dependencies and qualification

- added/removed dependencies;
- ADR/license/source/security qualification;
- fake/conformance fixture digest.

## Size

- hand-written Rust lines / largest module;
- split review required and decision.

## Residual state

- known failures/skips/blockers;
- contract/port change requests;
- follow-up integration work.

## Reviewer receipt

- ownership and dependency/port direction respected;
- security/currentness invariants preserved;
- tests reproduce;
- accept/reject with reasons.
