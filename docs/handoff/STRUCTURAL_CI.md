# Structural CI scope

Repository workflows are read-only, manual-only scaffold-integrity checks. They use commit-pinned
checkout, `contents: read`, `persist-credentials: false` and only `workflow_dispatch`.

## Validation layers

1. `validate-swarm.ps1` — Cargo members/manifests, package registry, assignments, dependency graph,
   launch counts/authorization, line limits and P00 manifest integrity.
2. `validate-function-packets.ps1` — exact 45-package closure across three foundation contracts and 42
   package-local `FUNCTIONS.md` packets, including assignment/wave/write-scope and operation-contract
   structure.
3. `validate-stage-readsets.ps1` — W0–W10 stage composition, **68 stage-package assignments**, gate/
   completion-receipt order and **23 later-stage replacement contexts** with a sixteen-file ceiling.
4. `validate-implementation-packets.ps1` — configuration/function packets and W3 Qdrant/lexical
   qualification shape.
5. `validate-current-packets.ps1` — W4 function/qualification registration plus W5 package links,
   baseline and 42 current-workspace probes.
6. `validate-w5-current.ps1` — W5 cross-contract, settings, bounded packets, Rust parser artifact/probes
   and exact W5/W6 partition of the central G3 evidence set.
7. `validate-proof-packets.ps1` — W6 resolver/comparator/exact links, locked non-overclaim baseline,
   unselected proof profiles, 52 mandatory probes and G3 evidence IDs.
8. `validate-w7-lifecycle.ps1` — W7 lifecycle/security packet, purge/restore/reclaim receipt separation.
9. `validate-w8-client-edge.ps1` — W8 generic edge, CLI/leaf authority floors and G4 mappings.
10. `validate-w9-product-pulse.ps1` — W9 Product Pulse corpus/metrics/probes and G5 mapping.
11. `validate-w10-optional-depth.ps1` — W10 candidate packages including candidate-specific
    `search-eval` reentry, disabled profiles/probes and G6 mapping.

The dedicated `Stage-specific read sets` workflow runs package/function validation first, then stage
closure, then stage-specific packet validators. It does not execute product behavior.

## What stage/read-set validation proves structurally

- every one of 45 packages appears at its exact registry earliest wave;
- `swarm/stages.toml` contains exactly W0 through W10 and 68 package assignments;
- W1/W2, W3/W4 and W5/W6 contribute to and close G1/G2/G3 in the intended pairs;
- W7 emits a separate `W7_LIFECYCLE` prerequisite rather than impersonating a central gate;
- W8 requires G3 plus W7 lifecycle, W9 requires G4 plus lifecycle, and W10 requires G5/P15;
- all stages after W0 remain `BLOCKED` and launch stays P00/W0;
- exactly 23 assignments reuse a package after its earliest wave;
- each reused package has one exact replacement override and an earliest-wave package has none;
- `base_stage` and immediate `prior_stage` match the stage history;
- accepted public prior-stage handoffs replace old stage documents and dependency implementation reads;
- exact W7 hardening, W8 protocol/daemon/CLI, W9 evaluation and W10 activation/scale/evaluation
  supplements are package-local;
- W10 `search-eval` consumes accepted W9/P15 handoffs rather than W4/W9 implementation packets;
- override write scopes match the immutable function registry;
- static context count is recomputed and does not exceed sixteen files;
- no stage read set includes the architecture master;
- launch state names the package/function/stage/read-set registries without advancing authorization;
- all workflows remain read-only and manual-only.

## What W5 validation proves structurally

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
- W5 owns seven currentness/overlay/parser G3 evidence items and W6 owns the remaining seven.

## Non-claims

The structural workflows do **not** prove:

- real Rust implementation or public API correctness;
- real Windows root ownership, ACL, secret store, watcher/USN or process containment;
- redb/CAS/Qdrant durability, security or fault recovery;
- overlay memory isolation or exhaustive runtime non-persistence;
- parser dependency safety, span behavior or no-execute containment;
- access/IDF noninterference, source readback or query correctness;
- publication, retention, purge, restore, handle or route-pin runtime behavior;
- client pairing/binding/protocol/CLI runtime behavior;
- latency, resource budgets, Product Pulse or optional-provider benefit;
- package, wave, central gate or product acceptance.

Passing is a merge prerequisite for scaffold changes, not a package/wave/gate receipt. Runtime claims
require exact commands, environment/artifact/API/profile identities, raw output and independent review.

Current pinned checkout identity:

```text
actions/checkout v7.0.1
commit 3d3c42e5aac5ba805825da76410c181273ba90b1
```
