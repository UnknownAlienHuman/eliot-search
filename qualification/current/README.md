# W5 current-workspace and Rust-structure qualification

This directory defines the P09–P10 evidence contract for:

- watcher/USN gap detection and authoritative reconciliation;
- truthful current-workspace preflight and live-head shadowing;
- saved and authenticated unsaved overlay precedence;
- exhaustive unsaved-byte non-persistence;
- one qualified no-execute Rust structural enrichment profile.

Files:

- [`W5_QUALIFICATION.md`](W5_QUALIFICATION.md) — execution order, owners, stop conditions and evidence.
- [`baseline.toml`](baseline.toml) — locked architecture/security/currentness assumptions.
- [`probes.toml`](probes.toml) — machine-readable mandatory probe registry.

All probe results are initially `UNAVAILABLE`. No watcher, IDE adapter or Rust parser artifact/provider is
selected or qualified by these documents. W5 remains blocked by `swarm/launch-state.toml`.
