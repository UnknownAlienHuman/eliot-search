# Structural CI scope

`Swarm structure` is a read-only scaffold-integrity workflow. It validates three layers:

1. `validate-swarm.ps1` — Cargo members/manifests, package registry, assignments, dependency graph,
   launch counts/authorization, line limits and P00 manifest integrity;
2. `validate-implementation-packets.ps1` — registry-declared function/configuration packets and W3
   Qdrant/lexical qualification shape;
3. `validate-current-packets.ps1` — W4 function/qualification registration plus W5 currentness,
   overlay and Rust-structure packet/baseline/probe integrity.

The workflow uses read-only repository permissions and a commit-pinned checkout action with credentials
not persisted.

It checks that:

- W4 query/protocol/eval package entries point to exact `FUNCTIONS.md` and W4 qualification packets;
- W5 reconcile/overlay/code-enricher entries point to exact function and W5 qualification packets;
- launch state remains P00/W0 and qualification paths match the registry;
- W5 baseline keeps watcher quietness, partial inventory, persistent unsaved bytes, stale fallback,
  compiler truth and parser execution paths disabled;
- parser/provider identity remains unselected and every W5 probe remains `UNAVAILABLE` before execution;
- mandatory currentness, shadowing, non-persistence, restart and Rust no-execute probes exist uniquely.

It does **not** prove:

- Rust implementation or contract correctness;
- actual Windows watcher/USN continuity;
- actual IDE buffer authentication or unsaved memory isolation;
- actual parser/process no-execute containment;
- Qdrant/redb/CAS fault behavior;
- security noninterference;
- latency, resource budgets or Product Pulse acceptance.

Passing is a merge prerequisite for scaffold changes, not a gate receipt. Runtime claims require exact
commands, environment/artifact identities, raw output and independent review.

Current pinned checkout identity:

```text
actions/checkout v7.0.1
commit 3d3c42e5aac5ba805825da76410c181273ba90b1
```
