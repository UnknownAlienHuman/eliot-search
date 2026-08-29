# W6 comparison and exact-proof qualification

This directory defines the P11–P12 evidence contract for:

- ambiguity-preserving subject resolution;
- descriptive cross-repository comparison with lineage independence and configuration variants;
- frozen-denominator exact scans and complete-negative proof semantics.

Files:

- [`W6_QUALIFICATION.md`](W6_QUALIFICATION.md) — execution order, owners, hard stops and evidence.
- [`baseline.toml`](baseline.toml) — locked non-normative and exact-proof invariants.
- [`profiles.toml`](profiles.toml) — resolver/comparison/exact engine/profile identities; initially
  unselected or schema-only.
- [`probes.toml`](probes.toml) — machine-readable mandatory probe registry.

All probe results start `UNAVAILABLE`. These files do not select a regex engine, parser profile or
subject/comparison policy implementation and do not authorize W6 implementation. Launch authority
remains `swarm/launch-state.toml`.
