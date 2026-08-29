# Authority and source-of-truth map

This map resolves conflicts between architecture, derived contract packs, machine registries and
package instructions.

## Precedence

1. **Architecture Part I** — product behavior, invariants, authority and security semantics.
2. **Accepted ADRs** — implementation/package decisions that do not change Part I.
3. **Accepted public API/schema/port digest** — actual contract consumed by downstream packages.
4. **P00 contract pack** — bounded derivative implementation projection; stops on Part I conflict.
5. **`swarm/crates.toml`** — exact package names, paths, direct dependencies, assignment paths,
   optionality, wave metadata and line targets.
6. **`swarm/launch-state.toml`** — current implementation authorization only.
7. **Package assignment** — package behavior, owned/forbidden state, operations, invariants and tests.
8. **Root/family/package `AGENTS.md`** — write scope and local operational rules.
9. **README/human matrix** — navigation and explanation only.

## Domain-specific authority

| Question | Authority |
|---|---|
| What Search is allowed to do | Architecture Part I |
| Exact shared fields and serialization after freeze | accepted `search-contracts` digest |
| Pure transition/order/coverage rules | accepted `search-domain` digest |
| Shared trait/method semantics | accepted `search-ports` digest |
| Exact direct Cargo dependencies | `swarm/crates.toml` plus matching Cargo manifest |
| May an agent start now | `swarm/launch-state.toml` |
| Which package owns mutable state | assignment + `PRIMITIVE_OWNERSHIP.md` |
| Which adapter implements a port | `PORT_CATALOG.md` and accepted adapter handoff |
| Which package owns a shared fixture | `tests/CRATE_FIXTURE_OWNERS.md` |

## Conflict handling

- A Part I conflict stops work with `CONTRACT_CHALLENGE`; derivative docs are not silently patched
  around it.
- Cargo and registry mismatch blocks merge; neither is treated as harmless documentation drift.
- Package `AGENTS.md` and assignment dependency prose are explanatory. The exact dependency closure is
  the package entry in `swarm/crates.toml`.
- An assignment cannot authorize a future wave; launch state cannot redefine package behavior.
- A README cannot add a field, port, reason code, dependency or authority.

## Freeze rule

Before a consumer starts, the integration owner records the producer commit and public API/schema/port
digest. Downstream writers consume that immutable receipt, not a moving branch or implementation
internals.
