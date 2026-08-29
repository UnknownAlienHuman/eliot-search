# Authority and source-of-truth map

This map resolves conflicts between architecture, derived contract packs, machine registries,
configuration/qualification packets and package instructions.

## Precedence

1. **Architecture Part I** — product behavior, invariants, authority and security semantics.
2. **Accepted ADRs / explicit correction documents** — implementation/package decisions that do not
   change Part I.
3. **Accepted public API/schema/port/configuration digest** — actual contract consumed downstream.
4. **P00 contract pack** — bounded derivative implementation projection; stops on Part I conflict.
5. **`swarm/crates.toml`** — exact package names, paths, direct dependencies, assignments,
   configuration/qualification packets, optionality, wave metadata and line targets.
6. **`swarm/function-packets.toml`** — exact primary function/contract packet and package-local write
   scope for every package; foundation packages point to their P00 contract files.
7. **`config/sections.toml`** — exact configuration section owner, earliest wave, minimum action, secret
   policy and bounded section packet.
8. **Qualification registry/packet** — exact artifact/probe/schema requirements; never a success receipt.
9. **`swarm/launch-state.toml`** — current implementation authorization only.
10. **Package assignment and function-registry primary packet** — owned behavior, operation semantics,
    failures, recovery, bounds and tests.
11. **Root/family/package `AGENTS.md`** — operational read/write rules within the machine registries.
12. **README/human matrix** — navigation and explanation only.

## Domain-specific authority

| Question | Authority |
|---|---|
| What Search is allowed to do | Architecture Part I |
| Exact shared fields and serialization after freeze | accepted `search-contracts` digest |
| Pure transition/order/coverage rules | accepted `search-domain` digest |
| Shared trait/method semantics | accepted `search-ports` digest |
| Generic configuration layering/redaction/planning | accepted `search-config` digest |
| Exact package path/dependencies/wave/assignment | `swarm/crates.toml` |
| Exact package function behavior packet and write scope | `swarm/function-packets.toml` |
| Which package owns a configuration section | `config/sections.toml` |
| Exact section fields/defaults/bounds/change obligations | section packet + accepted owner digest |
| Which stage packet and qualification apply | machine stage/swarm packet plus accepted ticket |
| May an agent start now | `swarm/launch-state.toml` |
| Which package owns mutable state | function packet + assignment + `PRIMITIVE_OWNERSHIP.md` |
| Which adapter implements a port | `PORT_CATALOG.md` and accepted adapter handoff |
| Is a Qdrant/provider/profile accepted | immutable qualification/evidence receipt |
| Which package owns a shared fixture | `tests/CRATE_FIXTURE_OWNERS.md` or stage fixture-owner registry |

## Conflict handling

- A Part I conflict stops work with `CONTRACT_CHALLENGE`; derivative docs are not silently patched.
- Cargo and `swarm/crates.toml` dependency mismatch blocks merge.
- Package name/path/wave/assignment mismatch between `swarm/crates.toml` and
  `swarm/function-packets.toml` blocks merge.
- Missing, duplicate, cross-package or structurally incomplete primary function packet blocks merge.
- Section owner/packet/Cargo dependency mismatch blocks merge.
- A qualification packet specifies what must be proven; empty/UNAVAILABLE evidence never enables a
  capability.
- Package `AGENTS.md` and assignment dependency prose are explanatory. Exact dependency closure is
  `swarm/crates.toml`; exact function/write closure is `swarm/function-packets.toml`.
- An assignment or function packet cannot authorize a future wave.
- A README cannot add a field, port, reason code, dependency, capability or authority.

## Bounded-context rule

An ordinary writer receives only root/package instructions, its exact package and function-registry
entries, one assignment, one primary function/contract packet, owned configuration/qualification/stage
packets, accepted direct handoffs and named fixtures.

The architecture master and another package's implementation internals are exception-only. A writer that
finds a missing load-bearing contract stops and opens a contract change; it does not widen its own read
set or infer behavior from implementation details.

## Freeze rule

Before a consumer starts, the integration owner records the producer commit and public
API/schema/port/configuration digest. Downstream writers consume that immutable receipt, not a moving
branch or implementation internals. An external-artifact consumer additionally receives the exact
accepted qualification receipt.
