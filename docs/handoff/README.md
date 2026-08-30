# Implementation handoff

## Machine authorities and launch procedure

- [AUTHORITY_MAP.md](AUTHORITY_MAP.md) — conflict precedence and source-of-truth ownership.
- [SWARM_LAUNCH_INDEX.md](SWARM_LAUNCH_INDEX.md) — deterministic context/ticket/lease construction,
  authorization preflight, write enforcement and review.
- [P00_DRAFT_CONTROL_PLANE.md](P00_DRAFT_CONTROL_PLANE.md) — non-claimable P00 ticket/context drafts,
  issuance-time materialization, writer leases, submissions and independent reviews.
- [TICKET_ISSUANCE_OPERATIONS.md](TICKET_ISSUANCE_OPERATIONS.md) — exact idempotent materialization,
  ticket/lease/submission/review/handoff operations, recovery decisions and typed failures.
- [SWARM_STAGE_READSETS.md](SWARM_STAGE_READSETS.md) — exact current-stage context assembly,
  replacement semantics, ceilings and progressive-package examples.
- [STAGE_READSET_AUDIT.md](STAGE_READSET_AUDIT.md) — current structural closure and non-claims.
- [`../../swarm/crates.toml`](../../swarm/crates.toml) — exact package path, dependency, earliest wave,
  assignment, configuration and qualification registry.
- [`../../swarm/function-packets.toml`](../../swarm/function-packets.toml) — exact primary function/
  contract packet and package-local write scope for all 45 packages.
- [`../../swarm/stages.toml`](../../swarm/stages.toml) — exact W0–W10 package composition, shared read
  sets and gate/completion-receipt order.
- [`../../swarm/stage-readsets.toml`](../../swarm/stage-readsets.toml) — replacement contexts for all 23
  package assignments reused after their earliest wave.
- [`../../swarm/orchestration.toml`](../../swarm/orchestration.toml) — issued-ticket, materialized-context,
  lease, submission, review, acceptance and wave-advance state machine.
- [`../../swarm/control-plane-schema.toml`](../../swarm/control-plane-schema.toml) — closed control-record
  schema registry, exact layouts, append-only rules, reason-type bindings and validator wiring.
- [`../../swarm/schemas/types-v1.toml`](../../swarm/schemas/types-v1.toml) — closed scalar/composite,
  failure, lease-event, supersession and consumer-action type registries.
- [`../../swarm/launch-state.toml`](../../swarm/launch-state.toml) — sole current implementation
  authorization.

## P00 draft and issued-record layouts

```text
swarm/ticket-drafts/p00/<package>.toml                         non-claimable draft
swarm/context-drafts/p00/<package>.toml                        unmaterialized context source list

swarm/context-manifests/<package>/<context_record_sha256>.toml immutable materialized context manifest
swarm/tickets/<package>/<ticket_id>.toml                       issued immutable ticket
swarm/leases/<package>/<lease_id>.toml                         writer lease
swarm/leases/<package>/events/<event_id>.toml                  append-only lease event
swarm/submissions/<package>/<submission_id>.toml               package submission
swarm/reviews/<package>/<review_id>.toml                       independent review
swarm/handoffs/<package>/<handoff_id>.toml                     accepted package/API handoff
swarm/supersessions/<record_kind>/<receipt_id>.toml            append-only replacement receipt
```

The valid progression is context materialization → ticket issuance → lease issuance → writer
acknowledgement → package-only implementation → submission → independent review → package handoff.
A draft, ticket or lease cannot imply a later record.

A package handoff is named by its unique `handoff_id`, not by `api_schema_digest`. The same public API
digest may legitimately survive multiple accepted implementation/evidence revisions, so API identity
cannot also be append-only record-path identity.

The current repository contains three P00 drafts and zero issued tickets, contexts, leases, submissions,
accepted reviews, supersessions or package handoffs. A draft never authorizes an agent.

## Structural validation

Run locally from the exact branch/worktree:

```powershell
pwsh -NoProfile -File tools/validate-swarm.ps1 -Json
pwsh -NoProfile -File tools/validate-function-packets.ps1 -Json
pwsh -NoProfile -File tools/validate-stage-readsets.ps1 -Json
pwsh -NoProfile -File tools/validate-p00-ticket-drafts.ps1 -Json
pwsh -NoProfile -File tools/validate-ticket-issuance-contracts.ps1 -Mode All -Json
pwsh -NoProfile -File tools/validate-implementation-packets.ps1 -Json
pwsh -NoProfile -File tools/validate-current-packets.ps1 -Json
pwsh -NoProfile -File tools/validate-w5-current.ps1 -Json
pwsh -NoProfile -File tools/validate-proof-packets.ps1 -Json
pwsh -NoProfile -File tools/validate-w7-lifecycle.ps1 -Json
pwsh -NoProfile -File tools/validate-w8-client-edge.ps1 -Json
pwsh -NoProfile -File tools/validate-w9-product-pulse.ps1 -Json
pwsh -NoProfile -File tools/validate-w10-optional-depth.ps1 -Json
```

The ticket-control validator uses a quote-aware TOML array scanner so selectors such as
`package[name=search-contracts]` cannot terminate an array early. It validates both the P00 zero state
and the closed issuance schemas. Passing this suite proves structural closure only; it is not Windows
runtime qualification, ticket/lease issuance, package acceptance, G0 or W0 evidence.

## Stage packets

- [`../contracts/p00/README.md`](../contracts/p00/README.md) — exact W0 contract implementation pack.
- [P00_BOOTSTRAP.md](P00_BOOTSTRAP.md) — draft issuance → contracts → domain/ports → W0 receipt sequence.
- [W1_IMPLEMENTATION_PACKET.md](W1_IMPLEMENTATION_PACKET.md) — configuration, root ownership, OS
  secrets, bounded control journal, provider protocol, daemon and CLI shell.
- [W2_IMPLEMENTATION_PACKET.md](W2_IMPLEMENTATION_PACKET.md) — source admission/identity/registry,
  stable no-execute reads, immutable revisions, materialization and unitization.
- [`../config/README.md`](../config/README.md) — configuration layering, ownership and composite
  reconfiguration.
- [W3_IMPLEMENTATION_PACKET.md](W3_IMPLEMENTATION_PACKET.md) — lexical/Qdrant/publication/pin/reclaim.
- [`../../qualification/qdrant/README.md`](../../qualification/qdrant/README.md) — unqualified
  P05–P07 artifact/schema/probe contract.
- [W4_IMPLEMENTATION_PACKET.md](W4_IMPLEMENTATION_PACKET.md) — access, bounded planning/execution,
  exact source validation, handles, results, continuations and evaluation seams.
- [`../../qualification/query/W4_QUALIFICATION.md`](../../qualification/query/W4_QUALIFICATION.md) —
  bounded P08 query evidence contract.
- [W5_IMPLEMENTATION_PACKET.md](W5_IMPLEMENTATION_PACKET.md) — currentness/overlay/Rust structure.
- [`../../qualification/current/README.md`](../../qualification/current/README.md) — unexecuted
  P09–P10 current-workspace evidence contract.
- [W6_IMPLEMENTATION_PACKET.md](W6_IMPLEMENTATION_PACKET.md) — subject resolution/comparison/exact proof.
- [`../../qualification/proof/README.md`](../../qualification/proof/README.md) — unexecuted P11–P12
  profile/baseline/probe contract.
- [W7_IMPLEMENTATION_PACKET.md](W7_IMPLEMENTATION_PACKET.md) — restrictive security, retention, purge
  and restore.
- [`../../qualification/lifecycle/README.md`](../../qualification/lifecycle/README.md) — unexecuted P13
  lifecycle evidence contract.
- [W8_IMPLEMENTATION_PACKET.md](W8_IMPLEMENTATION_PACKET.md) — generic client edge, standalone CLI and
  optional leaf profiles; the reused CLI delta is `bins/eliot-search/W8_CLIENT.md`.
- [`../../qualification/client-edge/README.md`](../../qualification/client-edge/README.md) — unexecuted
  P14 client-edge probe contract.
- [W9_IMPLEMENTATION_PACKET.md](W9_IMPLEMENTATION_PACKET.md) — Product Pulse/Windows evaluation.
- [`../../qualification/product-pulse/README.md`](../../qualification/product-pulse/README.md) —
  unexecuted P15 corpus/metric/probe/evidence contract.
- [W10_IMPLEMENTATION_PACKET.md](W10_IMPLEMENTATION_PACKET.md) — optional model/document/scale ownership,
  candidate-specific `search-eval` evidence, gate, migration and removal.
- [`../../qualification/optional-depth/README.md`](../../qualification/optional-depth/README.md) —
  disabled P16–P18 provider/topology templates and G6 probes.

## Audits and maps

- [STRUCTURAL_CI.md](STRUCTURAL_CI.md) — structural validators and non-claims.
- [SWARM_READINESS_AUDIT.md](SWARM_READINESS_AUDIT.md) — current readiness and honest status.
- [OWNERSHIP_BOUNDARY_AUDIT.md](OWNERSHIP_BOUNDARY_AUDIT.md) — missing-owner/dependency audit.
- [CRATE_MATRIX.md](CRATE_MATRIX.md) — one-agent/one-package human ownership index.
- [DEPENDENCY_GRAPH.md](DEPENDENCY_GRAPH.md) — semantic topology and daemon composition.
- [PORT_CATALOG.md](PORT_CATALOG.md) — shared port to concrete adapter mapping.
- [PRIMITIVE_OWNERSHIP.md](PRIMITIVE_OWNERSHIP.md) — schema, meaning, trait and mutable-state ownership.
- [IMPLEMENTATION_WAVES.md](IMPLEMENTATION_WAVES.md) — future dependency-safe sequence.

Part I Architecture 8.4 remains normative. Human docs, drafts, function/stage/read-set packets and
qualification designs never override exact machine registries, issued immutable tickets, accepted
API/evidence digests or launch state. `DRAFT_ONLY_NOT_ISSUED`, `UNMATERIALIZED_DRAFT`, `UNSELECTED`,
`UNQUALIFIED`, `UNAVAILABLE`, `DISABLED`, `BLOCKED` and `NOT_ACCEPTED` are explicit non-success states.

Every repository workflow is `workflow_dispatch`-only, uses read-only contents permission and disables
checkout credential persistence. A workflow run is structural evidence only; it never issues a ticket,
lease, package handoff, gate receipt or wave receipt.
