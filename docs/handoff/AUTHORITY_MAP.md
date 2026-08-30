# Authority and source-of-truth map

This map resolves conflicts between architecture, accepted public handoffs, machine registries,
configuration/qualification packets, stage contexts and package instructions.

## Precedence

1. **Architecture Part I** — product behavior, invariants, authority and security semantics.
2. **Accepted ADRs / explicit correction documents** — implementation/package decisions that do not
   change Part I.
3. **Accepted public API/schema/port/configuration/evidence digest** — actual immutable contract consumed
   by direct dependencies and later-stage writers.
4. **P00 contract pack** — bounded derivative implementation projection; stops on Part I conflict.
5. **`swarm/crates.toml`** — exact package names, paths, direct dependencies, assignments,
   configuration/qualification packets, optionality, earliest-wave metadata and line targets.
6. **`swarm/function-packets.toml`** — exact primary function/contract packet and package-local write
   scope for every package; foundation packages point to their P00 contract files.
7. **`swarm/stages.toml`** — exact W0–W10 package membership, shared current-stage context and
   gate/completion-receipt ordering.
8. **`swarm/stage-readsets.toml`** — exact replacement context for every package reused after its
   earliest wave.
9. **`config/sections.toml`** — exact configuration section owner, earliest wave, minimum action, secret
   policy and bounded section packet.
10. **Qualification registry/packet** — exact artifact/probe/schema requirements; never a success
    receipt.
11. **`swarm/launch-state.toml`** — current implementation authorization only.
12. **Package assignment and function-registry primary packet** — owned behavior, operation semantics,
    failures, recovery, bounds and tests.
13. **Current stage shared packet plus applicable later-stage supplement** — narrow additive obligations;
    cannot weaken the accepted base API.
14. **Root/family/package `AGENTS.md`** — operational read/write rules within the machine registries.
15. **README/human matrix** — navigation and explanation only.

## Domain-specific authority

| Question | Authority |
|---|---|
| What Search is allowed to do | Architecture Part I |
| Exact shared fields and serialization after freeze | accepted `search-contracts` digest |
| Pure transition/order/coverage rules | accepted `search-domain` digest |
| Shared trait/method semantics | accepted `search-ports` digest |
| Generic configuration layering/redaction/planning | accepted `search-config` digest |
| Exact package path/dependencies/earliest wave/assignment | `swarm/crates.toml` |
| Exact package function behavior packet and write scope | `swarm/function-packets.toml` |
| Which packages belong to a stage and what it contributes/closes | `swarm/stages.toml` |
| What a reused package reads at W7/W8/W9/W10 | `swarm/stage-readsets.toml` |
| Which package owns a configuration section | `config/sections.toml` |
| Exact section fields/defaults/bounds/change obligations | section packet + accepted owner digest |
| Which stage packet and qualification apply | `swarm/stages.toml` plus accepted ticket |
| Which earlier behavior a later writer may rely on | exact accepted prior-stage handoff/API digest |
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
- Stage package/phase/gate/receipt mismatch blocks merge.
- A package reused after its earliest wave without exactly one stage override blocks merge.
- An unnecessary override for an earliest-wave package blocks merge.
- A later-stage override that includes a forbidden previous-stage packet, dependency implementation or
  architecture master blocks merge.
- Section owner/packet/Cargo dependency mismatch blocks merge.
- A qualification packet specifies what must be proven; empty/UNAVAILABLE evidence never enables a
  capability.
- Package `AGENTS.md` and assignment dependency prose are explanatory. Exact dependency closure is
  `swarm/crates.toml`; exact function/write closure is `swarm/function-packets.toml`; exact stage context
  closure is `swarm/stages.toml` plus `swarm/stage-readsets.toml`.
- An assignment, function packet, stage entry or context override cannot authorize a future wave.
- A README cannot add a field, port, reason code, dependency, capability or authority.

## Bounded-context rule

An earliest-wave writer receives only root/package instructions, exact package/function/stage registry
entries, one assignment, one primary function/contract packet, current-stage shared files, accepted
direct handoffs and named fixtures.

A later-stage writer additionally receives its one exact override. An **accepted prior-stage handoff**
replaces the earlier stage packet and implementation history. The integration owner must not mount both
the prior implementation packet and the new replacement context.

Static context is capped at sixteen files. Ticket-added handoff receipts and fixture references are also
bounded. A writer that finds a missing load-bearing contract stops and opens a contract change; it does
not widen its own read set or inspect dependency internals.

## Stage/receipt rule

Central gate and stage-completion receipts remain distinct:

```text
W0 closes G0
W1 + W2 close G1
W3 + W4 close G2
W5 + W6 close G3
W7 emits W7_LIFECYCLE
W8 closes G4 and requires W7_LIFECYCLE
W9 closes G5 and requires G4 plus W7_LIFECYCLE
W10 closes candidate-specific G6 and requires accepted P15/G5
```

This prevents package/stage presence or a non-central lifecycle receipt from being mistaken for product
acceptance.

## Freeze rule

Before a consumer or later-stage writer starts, the integration owner records the producer commit and
public API/schema/port/configuration/evidence digest. Downstream agents consume that immutable receipt,
not a moving branch, previous stage packet or implementation internals. An external-artifact consumer
additionally receives the exact accepted qualification receipt.
