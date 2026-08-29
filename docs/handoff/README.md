# Implementation handoff

- [SWARM_READINESS_AUDIT.md](SWARM_READINESS_AUDIT.md) — current readiness and honest execution status.
- [OWNERSHIP_BOUNDARY_AUDIT.md](OWNERSHIP_BOUNDARY_AUDIT.md) — second-pass missing-owner/dependency audit.
- [CRATE_MATRIX.md](CRATE_MATRIX.md) — one-agent/one-package ownership and line budgets.
- [DEPENDENCY_GRAPH.md](DEPENDENCY_GRAPH.md) — package topology and progressive daemon composition.
- [PORT_CATALOG.md](PORT_CATALOG.md) — vendor-neutral I/O/state ports and concrete adapter ownership.
- [IMPLEMENTATION_WAVES.md](IMPLEMENTATION_WAVES.md) — dependency-safe launch order mapped to P00–P18.
- [P00_BOOTSTRAP.md](P00_BOOTSTRAP.md) — first contract/domain launch sequence.
- [PRIMITIVE_OWNERSHIP.md](PRIMITIVE_OWNERSHIP.md) — shared shape, pure meaning and runtime-state ownership.
- [SWARM_PROTOCOL.md](SWARM_PROTOCOL.md) — work isolation, contract-change and review rules.
- [`../../swarm/crates.toml`](../../swarm/crates.toml) — machine-readable package registry.
- [`../../tests/CRATE_FIXTURE_OWNERS.md`](../../tests/CRATE_FIXTURE_OWNERS.md) — shared fixture ownership.

These files refine implementation packaging only. They do not replace Architecture 8.4 invariants or
change the embedded architecture hash.
