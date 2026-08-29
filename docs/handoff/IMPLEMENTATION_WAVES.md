# Implementation waves

The architecture authorizes P00 first. `swarm/launch-state.toml` is the only current launch authority;
this document defines the future dependency-safe sequence. A wave description or packet is not an
assignment ticket.

## W0 — Contract freeze

1. `search-contracts` implements the exact P00 field-level pack.
2. Integration owner accepts its API/schema digest and resolves every contract challenge.
3. `search-domain` and `search-ports` may then run in parallel against that immutable handoff.
4. Integration owner pins the real Windows-compatible toolchain/dependencies, generates `Cargo.lock`,
   runs P00 policy/tests and publishes the W0 receipt.

No W1 package starts before that receipt.

## W1 — Process and control shell

Runtime owner, OS-bound secret adapter, bounded redb journal, generic frame/session shell and thin
CLI/daemon composition. Proves one-root ownership, secret non-disclosure, read-only hot admission,
framing limits and clean shutdown.

## W2 — Direct source spine

Source admission, identity/path history, registry/ownership, stable no-execute reads, residency-aware
revision CAS, text/code materialization and deterministic unitization. No index is required.

## W3 — Qualified lexical index

Exact Qdrant process artifact and containment, data-plane capability/schema gate, lexical fixtures,
point identity/projection manifests, linearizable publication, epoch pins and exact retired-point
reclamation.

Bounded packet: [`W3_IMPLEMENTATION_PACKET.md`](W3_IMPLEMENTATION_PACKET.md).

## W4 — Baseline query product

Pre-candidate access, server-owned plans, bounded leg execution, exact source-backed validation, handle
state, compact cards, continuations and raw read/grep evaluation baseline.

## W5 — Current workspace and code structure

Observation reconciliation, saved/unsaved overlays and qualified Rust structural enrichment.

Bounded packet: [`W5_IMPLEMENTATION_PACKET.md`](W5_IMPLEMENTATION_PACKET.md).

## W6 — Comparison and exact proof

Ambiguity-preserving subject resolution, descriptive cross-repository comparison and frozen-denominator
exact execution reports.

Bounded packet: [`W6_IMPLEMENTATION_PACKET.md`](W6_IMPLEMENTATION_PACKET.md).

## W7 — Security and lifecycle hardening

Restrictive-revocation linearization, durable handles, CAS mark-and-sweep, purge receipts/tombstones,
restore quarantine and ordinary-reclaim/purge separation.

Implementation remains blocked until all prerequisite package handoffs exist. Security, purge and
restore receipts cannot be inferred from ordinary query/reclaim behavior.

## W8 — Generic client edge

Full mutually authenticated binding/capability/evidence edge and standalone CLI. Optional ELIOT and
Research profiles remain leaf packages, disabled unless explicitly enabled and separately accepted.

Bounded packet: [`W8_IMPLEMENTATION_PACKET.md`](W8_IMPLEMENTATION_PACKET.md).

## W9 — Product Pulse / Windows qualification

One exact Windows environment, immutable control corpus, pinned A/B baselines, accepted
DIRECT/LEXICAL/CODE candidate, pre-registered criteria, paired randomized execution, latency/resource/
fault/protocol/leakage evidence and independent Product Pulse verdict.

Bounded packet: [`W9_IMPLEMENTATION_PACKET.md`](W9_IMPLEMENTATION_PACKET.md). Machine packet:
[`../../swarm/w9-product-pulse.toml`](../../swarm/w9-product-pulse.toml).

W9 starts only after G4 plus lifecycle/security prerequisites. Compilation, unit tests, screenshots,
prose or post-hoc thresholds do not pass G5. Only exact `ACCEPTED` plus independent reviewer receipt may
be consumed by W10.

## W10 — Optional depth

W10 contains three independently selectable candidate classes:

```text
P16 model optional
  - rerank-only
  - dense vector
  - multivector

P17 document optional
  - one exact isolated no-execute document provider profile

P18 advanced scale optional
  - one exact measured Qdrant topology/profile migration
```

Bounded packet: [`W10_IMPLEMENTATION_PACKET.md`](W10_IMPLEMENTATION_PACKET.md). Machine packet:
[`../../swarm/w10-optional-depth.toml`](../../swarm/w10-optional-depth.toml).

Each integration ticket selects **one** candidate/profile and requires:

1. exact accepted P15 report and reviewer receipt;
2. one dedicated candidate ADR;
3. exact Windows artifact/runtime/profile/license qualification;
4. compiled non-default feature plus explicit configuration and binding authorization;
5. pre-registered measured material incremental benefit;
6. tested removal restoring accepted P15 behavior;
7. migration/rollback evidence, or independently reviewed rerank-only no-persistent-state proof.

Package order is candidate-specific:

```text
model:
  search-model-provider
  -> eliot-search-model-worker
  -> daemon staging
  -> generation migration when persistent vectors exist
  -> incremental Product Pulse
  -> removal/rollback
  -> G6 review

document:
  eliot-search-doc-worker sandbox/provider shell
  -> exact provider qualification
  -> candidate representation/projection generation
  -> incremental Product Pulse
  -> removal/rollback
  -> G6 review

scale:
  measured one-shard bottleneck
  -> qdrant bridge topology qualification
  -> publication R0/catch-up/R1 migration
  -> guarded redb route switch
  -> route-pin drain and exact old-route reclaim
  -> benefit/rollback evidence
  -> G6 review
```

No candidate is selected in the scaffold. Feature presence, configuration, worker readiness, model name,
provider popularity or green unit tests cannot authorize serving. Optional failure must leave the
accepted P15 baseline available.

## Launch rule

For each package the orchestrator verifies `swarm/crates.toml`, launch state, the wave-specific packet,
accepted dependency API/port/configuration digests and any artifact/evidence receipts. It creates one
isolated worktree, provides only the bounded read set, rejects writes outside package scope and merges in
topological order.

Shared Cargo/dependency/profile/qualification/evidence changes belong to the integration owner. Package
writers cannot select external providers, enable optional profiles or self-accept a gate.
