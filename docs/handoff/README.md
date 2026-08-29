# Implementation handoff

- [`../contracts/p00/README.md`](../contracts/p00/README.md) — exact W0 contract implementation pack.
- [P00_BOOTSTRAP.md](P00_BOOTSTRAP.md) — contracts → domain/ports → W0 receipt sequence.
- [SWARM_READINESS_AUDIT.md](SWARM_READINESS_AUDIT.md) — current readiness and honest execution status.
- [OWNERSHIP_BOUNDARY_AUDIT.md](OWNERSHIP_BOUNDARY_AUDIT.md) — missing-owner/dependency audit.
- [CRATE_MATRIX.md](CRATE_MATRIX.md) — one-agent/one-package ownership and line budgets.
- [DEPENDENCY_GRAPH.md](DEPENDENCY_GRAPH.md) — package topology and progressive daemon composition.
- [PORT_CATALOG.md](PORT_CATALOG.md) — shared port ownership and concrete adapter mapping.
- [PRIMITIVE_OWNERSHIP.md](PRIMITIVE_OWNERSHIP.md) — schema, meaning, trait and mutable-state ownership.
- [IMPLEMENTATION_WAVES.md](IMPLEMENTATION_WAVES.md) — dependency-safe launch order.
- [SWARM_PROTOCOL.md](SWARM_PROTOCOL.md) — work isolation and contract-change rules.
- [`../../swarm/crates.toml`](../../swarm/crates.toml) — machine package registry.

These files refine implementation packaging and P00 projection only; Part I Architecture 8.4 remains
normative.
