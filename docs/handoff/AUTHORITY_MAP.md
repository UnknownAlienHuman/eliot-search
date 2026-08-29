# Authority and source-of-truth map

This map resolves conflicts between architecture, derived contract packs, machine registries,
configuration/qualification packets and package instructions.

## Precedence

1. **Architecture Part I** — product behavior, invariants, authority and security semantics.
2. **Accepted ADRs / explicit correction documents** — implementation/package decisions that do not
   change Part I.
3. **Accepted public API/schema/port/configuration digest** — actual contract consumed downstream.
4. **P00 contract pack** — bounded derivative implementation projection; stops on Part I conflict.
5. **`swarm/crates.toml`** — exact package names, paths, direct dependencies, assignment, function,
   configuration and qualification packet paths, optionality, wave metadata and line targets.
6. **`config/sections.toml`** — exact configuration section owner, earliest wave, minimum action, secret
   policy and bounded section packet.
7. **Qualification registry/packet** — exact artifact/probe/schema requirements; never a success receipt.
8. **`swarm/launch-state.toml`** — current implementation authorization only.
9. **Package assignment and registry-declared `FUNCTIONS.md`** — owned behavior, operation semantics,
   failures, recovery, bounds and tests.
10. **Root/family/package `AGENTS.md`** — write scope and local operational rules.
11. **README/human matrix** — navigation and explanation only.

## Domain-specific authority

| Question | Authority |
|---|---|
| What Search is allowed to do | Architecture Part I |
| Exact shared fields and serialization after freeze | accepted `search-contracts` digest |
| Pure transition/order/coverage rules | accepted `search-domain` digest |
| Shared trait/method semantics | accepted `search-ports` digest |
| Generic configuration layering/redaction/planning | accepted `search-config` digest |
| Which package owns a configuration section | `config/sections.toml` |
| Exact section fields/defaults/bounds/change obligations | section packet + accepted owner digest |
| Exact direct Cargo dependencies and bounded read packets | `swarm/crates.toml` plus matching files |
| May an agent start now | `swarm/launch-state.toml` |
| Which package owns mutable state | assignment + `PRIMITIVE_OWNERSHIP.md` |
| Which adapter implements a port | `PORT_CATALOG.md` and accepted adapter handoff |
| Is a Qdrant artifact/profile accepted | immutable P05–P07 qualification/evidence receipt |
| Which package owns a shared fixture | `tests/CRATE_FIXTURE_OWNERS.md` |

## Conflict handling

- A Part I conflict stops work with `CONTRACT_CHALLENGE`; derivative docs are not silently patched.
- Cargo and registry mismatch blocks merge; neither is harmless documentation drift.
- Section owner/packet/Cargo dependency mismatch blocks merge.
- A qualification packet specifies what must be proven; empty/UNAVAILABLE evidence never enables a
  capability.
- Package `AGENTS.md` and assignment dependency prose are explanatory. Exact dependency/read-set closure
  is the package entry in `swarm/crates.toml`.
- An assignment or function packet cannot authorize a future wave.
- A README cannot add a field, port, reason code, dependency, capability or authority.

## Freeze rule

Before a consumer starts, the integration owner records the producer commit and public
API/schema/port/configuration digest. Downstream writers consume that immutable receipt, not a moving
branch or implementation internals. An external-artifact consumer additionally receives the exact
accepted qualification receipt.
