# Implementation handoff

- [AUTHORITY_MAP.md](AUTHORITY_MAP.md) — conflict precedence and source-of-truth ownership.
- [`../contracts/p00/README.md`](../contracts/p00/README.md) — exact W0 contract implementation pack.
- [P00_BOOTSTRAP.md](P00_BOOTSTRAP.md) — contracts → domain/ports → W0 receipt sequence.
- [SWARM_READINESS_AUDIT.md](SWARM_READINESS_AUDIT.md) — current readiness and honest status.
- [OWNERSHIP_BOUNDARY_AUDIT.md](OWNERSHIP_BOUNDARY_AUDIT.md) — missing-owner/dependency audit.
- [CRATE_MATRIX.md](CRATE_MATRIX.md) — one-agent/one-package human ownership index.
- [DEPENDENCY_GRAPH.md](DEPENDENCY_GRAPH.md) — semantic topology and daemon composition.
- [PORT_CATALOG.md](PORT_CATALOG.md) — shared port to concrete adapter mapping.
- [PRIMITIVE_OWNERSHIP.md](PRIMITIVE_OWNERSHIP.md) — schema, meaning, trait and mutable-state ownership.
- [IMPLEMENTATION_WAVES.md](IMPLEMENTATION_WAVES.md) — future dependency-safe sequence.
- [`../../swarm/crates.toml`](../../swarm/crates.toml) — exact machine package/dependency registry.

Part I Architecture 8.4 remains normative. Human docs never override registry dependencies, accepted
API digests or launch state.
