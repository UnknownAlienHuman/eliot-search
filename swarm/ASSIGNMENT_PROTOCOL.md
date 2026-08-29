# Package assignment protocol

Every package writer reads this file together with root/family/package `AGENTS.md`, exactly one
`assignments/<package>.md`, and accepted public dependency handoffs.

## Boundaries

- One active writer, one Cargo package, one isolated worktree.
- Writer edits only the package path named in the assignment.
- Root workspace, lockfile, toolchain, CI, `swarm/`, architecture, generated schemas and shared fixtures
  belong to the integration owner.
- Dependency internals and future-wave packages are outside the read set.
- Missing/contradictory load-bearing semantics use `CONTRACT_CHANGE_TEMPLATE.md`; never invent a local
  duplicate type or silent fallback.

## Operation contract

Every logical operation in an assignment must define validated inputs, output identity, preconditions,
postconditions, typed failures, and cancellation/deadline behavior where applicable. Partial/degraded
outcomes are data, not apparent success.

## Implementation order

1. Turn package invariants into failing local tests.
2. Define vendor-neutral public ports and opaque package-owned state.
3. Add deterministic happy-path fixtures.
4. Add required negative/property/fault/security/qualification tests.
5. Implement the smallest behavior that closes them.
6. Complete `PACKAGE_HANDOFF_TEMPLATE.md` with raw commands/output and exact unresolved requests.

## Dependency and evidence rules

- Consume accepted public ports; do not reinterpret dependency-owned state.
- Vendor structs stay inside their adapter package.
- New external dependencies require exact version/source/license review and, when load-bearing, an ADR.
- No wildcard/floating git dependency and no Python/Node production dependency in the baseline.
- Compilation alone is not acceptance. Windows/Qdrant/redb/provider claims require executed
  environment/artifact identity; unavailable checks remain unavailable.

## Size

The assignment gives an initial target and split rule. Design/split review is mandatory before 8,500
total hand-written Rust lines; 10,000 including package-local tests is a hard stop. Splits require a real
dependency/replacement/security/runtime/test/context boundary; forwarding-only crates are forbidden.
