# Package assignment protocol

Every package writer reads this file together with root/family/package `AGENTS.md`, exactly one
`assignments/<package>.md`, [`../docs/handoff/PORT_CATALOG.md`](../docs/handoff/PORT_CATALOG.md) and
accepted public dependency handoffs.

## Boundaries

- One active writer, one Cargo package, one isolated worktree.
- Writer edits only the package path named in the assignment.
- Root workspace, lockfile, toolchain, CI, `swarm/`, architecture, generated schemas and shared fixtures
  belong to the integration owner.
- Dependency internals and future-wave packages are outside the read set.
- Missing/contradictory load-bearing semantics use `CONTRACT_CHANGE_TEMPLATE.md`; never invent a local
  duplicate type, adapter or silent fallback.

## Operation contract

Every logical operation defines validated inputs, output identity, preconditions, postconditions,
typed failures, and cancellation/deadline behavior where applicable. Partial/degraded outcomes are data,
not apparent success.

## Port rule

- Shared wire/domain records live in `search-contracts`; pure meaning lives in `search-domain`.
- A capability owns its public vendor-neutral port semantics and mutable state.
- Concrete redb, OS-secret, Qdrant process and Qdrant data-plane adapters are constructed only by
  `eliot-searchd`.
- A consumer may depend on an accepted capability API but cannot open the producer's storage or
  reinterpret its state.
- Vendor structs, raw collection names, point IDs, credentials and authorization decisions never cross
  public ports.

## Implementation order

1. Turn package invariants into failing local tests.
2. Define or consume the assigned vendor-neutral ports and opaque package-owned state.
3. Add deterministic happy-path fixtures.
4. Add required negative/property/fault/security/qualification tests.
5. Implement the smallest behavior that closes them.
6. Complete `PACKAGE_HANDOFF_TEMPLATE.md` with API/port digest, raw commands and unresolved requests.

## Dependency and evidence rules

- Consume accepted public ports; do not reinterpret dependency-owned state.
- New external dependencies require exact version/source/license review and, when load-bearing, an ADR.
- No wildcard/floating git dependency and no Python/Node production dependency in the baseline.
- Compilation alone is not acceptance. Windows/Qdrant/redb/provider claims require executed
  environment/artifact identity; unavailable checks remain unavailable.

## Size

The assignment gives an initial target and split rule. Design/split review is mandatory before 8,500
total hand-written Rust lines; 10,000 including package-local tests is a hard stop. Splits require a real
dependency, replacement, security, runtime, test or context boundary; forwarding-only crates are
forbidden.
