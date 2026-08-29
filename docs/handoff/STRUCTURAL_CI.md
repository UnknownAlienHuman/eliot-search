# Structural CI scope

`Swarm structure` is a read-only scaffold-integrity workflow. It validates four layers:

1. `validate-swarm.ps1` — Cargo members/manifests, package registry, assignments, dependency graph,
   launch counts/authorization, line limits and P00 manifest integrity;
2. `validate-implementation-packets.ps1` — registry-declared function/configuration packets and W3
   Qdrant/lexical qualification shape;
3. `validate-current-packets.ps1` — W4 function/qualification registration plus W5 currentness,
   overlay and Rust-structure packet/baseline/probe integrity;
4. `validate-proof-packets.ps1` — W6 resolver/comparator/exact function links, locked non-overclaim
   baseline, unselected proof profiles, 52 mandatory probes and G3 evidence IDs.

The workflow uses read-only repository permissions and a commit-pinned checkout action with credentials
not persisted.

It checks that:

- registry and launch-state qualification paths remain synchronized through W6;
- W6 resolver/comparator/exact entries point to exact package-local `FUNCTIONS.md` and one qualification
  packet;
- launch state remains P00/W0 with only `search-contracts` authorized;
- material ambiguity, lineage independence, evidence-role separation and non-normative output remain
  locked;
- Qdrant/top-k/client file lists cannot become exact denominators;
- every exact denominator item must be accounted and incomplete conditions cannot satisfy complete
  negative proof;
- regex engine/provider and structural exact profile identities remain `UNSELECTED` before qualification;
- all 52 W6 probes remain unique, mandatory and `UNAVAILABLE` before execution;
- G3 names separate evidence for ambiguity/drift, lineage/cfg variants, non-normative coverage, predicate
  qualification, frozen denominator, incomplete failures and security/unsaved revalidation.

It does **not** prove:

- Rust implementation or contract correctness;
- subject resolution quality on real repositories;
- actual repository lineage/copy independence;
- actual regex safety/performance or structural exact behavior;
- exact inventory/revision/access/overlay runtime integration;
- complete-negative proof correctness under real faults;
- Qdrant/redb/CAS/Windows security behavior;
- latency, resource budgets or Product Pulse acceptance.

Passing is a merge prerequisite for scaffold changes, not a gate receipt. Runtime claims require exact
commands, environment/artifact/API/profile identities, raw output and independent review.

Current pinned checkout identity:

```text
actions/checkout v7.0.1
commit 3d3c42e5aac5ba805825da76410c181273ba90b1
```
