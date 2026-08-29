# Structural CI scope

`Swarm structure` is a read-only, manual-only scaffold-integrity workflow. It validates five layers:

1. `validate-swarm.ps1` — Cargo members/manifests, package registry, assignments, dependency graph,
   launch counts/authorization, line limits and P00 manifest integrity;
2. `validate-implementation-packets.ps1` — registry-declared function/configuration packets and W3
   Qdrant/lexical qualification shape;
3. `validate-current-packets.ps1` — W4 function/qualification registration plus W5 package links,
   baseline and 42 current-workspace probes;
4. `validate-w5-current.ps1` — W5 cross-contract, settings, bounded read/write packets, Rust parser
   artifact/probes and exact W5/W6 partition of the central G3 evidence set;
5. `validate-proof-packets.ps1` — W6 resolver/comparator/exact links, locked non-overclaim baseline,
   unselected proof profiles, 52 mandatory probes and G3 evidence IDs.

The workflows use read-only repository permissions, commit-pinned checkout and
`persist-credentials: false`. They run only through `workflow_dispatch`.

The W5 checks prove structurally that:

- watcher/USN observations are hints and cannot confirm currentness;
- overflow/reset/rebind opens a gap before acknowledgement;
- partial/cancelled/timed-out inventory cannot close a gap or remove unseen sources;
- disk, saved revision, buffer snapshot and projection currentness remain separate;
- unsaved bytes cannot enter redb, CAS, Qdrant, logs, metrics, backups, crash artifacts, provider caches,
  evaluation corpora or learning inputs;
- overlay shadowing occurs before retrieval and IDF and cannot fall back to stale base on failure;
- durable handles/continuations cannot target unsaved bytes;
- Rust parser/provider remains `UNSELECTED`/`UNQUALIFIED`;
- Cargo, rustc, build scripts, macro expansion, network/package resolution and compiler-semantic overclaim
  remain disabled;
- all 42 current-workspace and 17 Rust-syntax probes remain mandatory and unavailable before execution;
- W5 owns only seven currentness/overlay/parser G3 evidence items; downstream W6 owns the remaining seven;
- launch authority remains P00/W0.

The structural workflows do **not** prove:

- real Windows watcher/USN continuity or IDE buffer authentication;
- memory isolation or exhaustive runtime non-persistence;
- parser dependency safety, span behavior or no-execute containment;
- reconciliation, shadowing or currentness correctness under real faults;
- Rust implementation or API correctness;
- Qdrant/redb/CAS/Windows security behavior;
- comparison/exact-proof quality;
- latency, resource budgets or Product Pulse acceptance.

Passing is a merge prerequisite for scaffold changes, not a package, wave or product receipt. Runtime
claims require exact commands, environment/artifact/API/profile identities, raw output and independent
review.

Current pinned checkout identity:

```text
actions/checkout v7.0.1
commit 3d3c42e5aac5ba805825da76410c181273ba90b1
```
