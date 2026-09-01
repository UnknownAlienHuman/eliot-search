# Authority and source-of-truth map

This map resolves conflicts between architecture, accepted public handoffs, machine registries,
configuration/qualification packets, stage contexts, assignment control records and package
instructions.

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
7. **`swarm/module-packets.toml` plus `swarm/modules/*.toml`** — exact package-local logical modules and
   public entry module for all 45 packages.
8. **`swarm/coverage/manifest.toml` plus `swarm/coverage/*.toml`** — integration-owned derivative
   crosswalk proving every architecture section, capability, invariant, port, type/schema, recipe,
   reason, assignment and delivery slice has concrete package/module owners. It cannot override items
   1–7.
9. **`swarm/stages.toml`** — exact W0–W10 package membership, shared current-stage context and
   gate/completion-receipt ordering.
10. **`swarm/stage-readsets.toml`** — exact replacement context for every package reused after its
    earliest wave.
11. **`config/sections.toml`** — exact configuration section owner, earliest wave, minimum action, secret
    policy and bounded section packet.
12. **Qualification registry/packet** — exact artifact/probe/schema requirements; never a success
    receipt.
13. **`swarm/launch-state.toml`** — current package authorization/conditional status only.
14. **Issued immutable assignment ticket, materialized context manifest and active writer lease** — exact
    writer/base/read/write/dependency/evidence fence for one package implementation.
15. **Package assignment and function-registry primary packet** — owned behavior, operation semantics,
    failures, recovery, bounds and tests.
16. **Current stage shared packet plus applicable later-stage supplement** — narrow additive obligations;
    cannot weaken the accepted base API.
17. **Root/family/package `AGENTS.md`** — operational read/write rules within the machine registries and
    issued ticket.
18. **README/human matrix and non-claimable ticket/context drafts** — preparation/navigation only.

`swarm/ticket-drafts/**` and `swarm/context-drafts/**` have **no implementation authority**. They do not
enter the orchestration state machine, create a lease or identify a base commit. Issuance always creates
new immutable records under `swarm/tickets/`, `swarm/context-manifests/` and `swarm/leases/`.

The coverage crosswalk is an integration audit and merge guard. A row does not authorize its package,
create implementation state or accept evidence. A conflict with Architecture Part I, an accepted ADR or
an accepted public digest invalidates the coverage row and stops work.

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
| Exact package-local logical modules and public entry | `swarm/module-packets.toml` plus the referenced wave packet |
| Does every normative section/cell/invariant/port/schema/recipe/task have an owner | `swarm/coverage/manifest.toml` and referenced coverage packets, validated against source |
| Which packages belong to a stage and what it contributes/closes | `swarm/stages.toml` |
| What a reused package reads at W7/W8/W9/W10 | `swarm/stage-readsets.toml` |
| Which package owns a configuration section | `config/sections.toml` |
| Exact section fields/defaults/bounds/change obligations | section packet + accepted owner digest |
| Which stage packet and qualification apply | `swarm/stages.toml` plus issued ticket |
| Which earlier behavior a later writer may rely on | exact accepted prior-stage handoff/API digest |
| May a ticket be considered for issuance | `swarm/launch-state.toml` plus accepted prerequisites |
| What exact context and writer may act | issued ticket + materialized context + active lease |
| Does a committed ticket/context draft authorize work | never; draft status is non-claimable |
| Which package owns mutable state | function packet + module packet + assignment + `PRIMITIVE_OWNERSHIP.md` |
| Which adapter implements a shared port | `swarm/coverage/ports.toml`, `PORT_CATALOG.md` and accepted adapter handoff |
| Is a Qdrant/provider/profile accepted | immutable qualification/evidence receipt |
| Which package owns a shared fixture | `tests/CRATE_FIXTURE_OWNERS.md` or stage fixture-owner registry |

## Conflict handling

- A Part I conflict stops work with `CONTRACT_CHALLENGE`; derivative docs are not silently patched.
- Cargo and `swarm/crates.toml` dependency mismatch blocks merge.
- Package name/path/wave/assignment mismatch between `swarm/crates.toml` and
  `swarm/function-packets.toml` blocks merge.
- Missing, duplicate, cross-package or structurally incomplete primary function packet blocks merge.
- Missing, duplicate, invalid or cross-package module packet blocks merge.
- An operation without one registered package owner and package public-entry module blocks merge.
- An S0–S39 section, C00–C30 capability, INV-01–INV-30 invariant, shared port, named P00 type/schema,
  recipe, reason namespace, package assignment or P00–P18 delivery slice without valid package/module
  ownership blocks merge.
- A shared port with a floating implementation owner such as “selected implementation” or “runtime
  adapter” blocks merge; one exact package/module is required.
- A schema with no shape owner, or mutable state with no state owner, blocks merge.
- A package absent from every architecture delivery slice blocks merge.
- Stage package/phase/gate/receipt mismatch blocks merge.
- A package reused after its earliest wave without exactly one stage override blocks merge.
- An unnecessary override for an earliest-wave package blocks merge.
- A later-stage override that includes a forbidden previous-stage packet, dependency implementation or
  architecture master blocks merge.
- Section owner/packet/Cargo dependency mismatch blocks merge.
- A qualification packet specifies what must be proven; empty/UNAVAILABLE evidence never enables a
  capability.
- A draft placed in an issued-ticket/context/lease directory, or a draft with selected writer/base/
  digest/lease fields, blocks merge.
- An issued ticket without one exact materialized context, writer/reviewer separation, registry/
  instruction digests and prerequisite handoffs is invalid.
- A package submission without a complete package-only diff, raw outcomes and matching ticket/context/
  lease identities cannot enter review.
- A review prepared by the writer or represented as a gate/wave receipt is invalid.
- Package `AGENTS.md` and assignment dependency prose are explanatory. Exact dependency closure is
  `swarm/crates.toml`; exact function/write closure is `swarm/function-packets.toml`; exact module
  closure is `swarm/module-packets.toml`; exact stage context closure is `swarm/stages.toml` plus
  `swarm/stage-readsets.toml`.
- An assignment, function packet, module packet, coverage row, stage entry, context override or draft
  cannot authorize a future wave.
- A README cannot add a field, port, reason code, dependency, capability or authority.

## Bounded-context rule

An earliest-wave writer receives only root/package instructions, exact package/function/module/stage
registry entries, one assignment, one primary function/contract packet, current-stage shared files,
accepted direct handoffs and named fixtures.

Before the writer receives anything, the integration owner materializes the exact source files and
registry fragments declared by the context draft at one base commit, records every source/fragment
SHA-256 and publishes one immutable context artifact. The exact package entry from the applicable
`swarm/modules/*.toml` packet is mandatory; the writer must not infer its module layout from another
package's source.

A later-stage writer additionally receives its one exact override. An **accepted prior-stage handoff**
replaces the earlier stage packet and implementation history. The integration owner must not mount both
the prior implementation packet and the new replacement context.

Static context is capped at sixteen files before declared context materialization. Ticket-added handoff
receipts and fixture references are also bounded. A writer that finds a missing load-bearing contract
stops and opens a contract change; it does not widen its own read set or inspect dependency internals.

## Ticket, submission and review rule

```text
non-claimable draft
→ new issued ticket + materialized context
→ active writer lease
→ package-only submission
→ independent review
→ append-only accepted package/API handoff
```

Changing the base commit, context source, assignment, registry, dependency handoff, module packet or
writer creates a superseding ticket/context/lease. A writer cannot amend its context, edit control-plane
records, self-review, self-accept or advance launch state.

A package review permits the integration owner to construct a package handoff only. It is not a gate or
wave receipt.

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

This prevents package/stage/draft presence or a non-central lifecycle receipt from being mistaken for
product acceptance.

## Freeze rule

Before a consumer or later-stage writer starts, the integration owner records the producer commit and
public API/schema/port/configuration/evidence digest. Downstream agents consume that immutable receipt,
not a moving branch, previous stage packet or implementation internals. An external-artifact consumer
additionally receives the exact accepted qualification receipt.
