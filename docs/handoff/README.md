# Implementation handoff

## Machine authorities

- [AUTHORITY_MAP.md](AUTHORITY_MAP.md) — conflict precedence and source-of-truth ownership.
- [`../../swarm/crates.toml`](../../swarm/crates.toml) — exact package path, dependency, wave,
  assignment, configuration and qualification registry.
- [`../../swarm/function-packets.toml`](../../swarm/function-packets.toml) — exact primary function/
  contract packet and package-local write scope for all 45 packages.
- [`../../swarm/launch-state.toml`](../../swarm/launch-state.toml) — sole current implementation
  authorization.

## Stage packets

- [`../contracts/p00/README.md`](../contracts/p00/README.md) — exact W0 contract implementation pack.
- [P00_BOOTSTRAP.md](P00_BOOTSTRAP.md) — contracts → domain/ports → W0 receipt sequence.
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
  optional leaf profiles.
- [`../../qualification/client-edge/README.md`](../../qualification/client-edge/README.md) — unexecuted
  P14 client-edge probe contract.
- [W9_IMPLEMENTATION_PACKET.md](W9_IMPLEMENTATION_PACKET.md) — Product Pulse/Windows evaluation.
- [`../../qualification/product-pulse/README.md`](../../qualification/product-pulse/README.md) —
  unexecuted P15 corpus/metric/probe/evidence contract.
- [W10_IMPLEMENTATION_PACKET.md](W10_IMPLEMENTATION_PACKET.md) — optional model/document/scale ownership,
  gate, migration and removal.
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

Part I Architecture 8.4 remains normative. Human docs, function packets and qualification designs never
override exact dependency/function registries, accepted API digests, executed evidence or launch state.
`UNSELECTED`, `UNQUALIFIED`, `UNAVAILABLE`, `DISABLED`, `BLOCKED` and `NOT_ACCEPTED` are explicit
non-success states.
