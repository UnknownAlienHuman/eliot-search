# W7 lifecycle, purge and restore qualification

This directory defines the P13 evidence contract for:

- monotonic restrictive security publication and revocation checkpoints;
- durable handle/continuation authorization and lifecycle invalidation;
- residency-aware CAS mark/sweep with active-pin protection;
- multi-layer purge with live fences and non-resurrection tombstones;
- paired recovery manifests, restore quarantine and guarded new-route admission;
- strict separation between ordinary index reclaim, CAS retention and security/legal purge.

Files:

- [`W7_QUALIFICATION.md`](W7_QUALIFICATION.md) — execution order, owners, hard stops and evidence.
- [`baseline.toml`](baseline.toml) — locked security/lifecycle/non-overclaim invariants.
- [`probes.toml`](probes.toml) — 60 machine-readable mandatory probes.
- [`fixture-owners.toml`](fixture-owners.toml) — exact shared-corpus owner map for W7.

Every probe starts `UNAVAILABLE`. These documents select no backup provider and execute no deletion,
purge or restore. W7 remains blocked by `swarm/launch-state.toml`.
