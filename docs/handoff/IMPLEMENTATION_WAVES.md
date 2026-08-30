# Implementation waves

The architecture authorizes P00 first. `swarm/launch-state.toml` is the only current launch authority;
this document explains the future dependency-safe sequence.

Machine truth for future ticket construction is split across:

```text
swarm/crates.toml             package dependencies and earliest wave
swarm/function-packets.toml   base function/contract and package write scope
swarm/stages.toml             exact W0–W10 package sets and gate/receipt order
swarm/stage-readsets.toml     replacement context for packages returning later
```

A wave description, stage entry, read-set override or package packet is not an assignment ticket.

## W0 — Contract freeze

1. `search-contracts` implements the exact P00 field-level pack.
2. Integration owner accepts its API/schema digest and resolves every contract challenge.
3. `search-domain` and `search-ports` may then run in parallel against that immutable handoff.
4. Integration owner pins the real Windows-compatible toolchain/dependencies, generates `Cargo.lock`,
   runs P00 policy/tests and publishes the W0/G0 receipt.

No W1 package starts before that receipt.

## W1 — Process and control shell

Runtime owner, OS-bound secret adapter, bounded redb journal, generic frame/session shell and thin
CLI/daemon composition. Proves one-root ownership, secret non-disclosure, read-only hot admission,
framing limits and clean shutdown.

Bounded packet: [`W1_IMPLEMENTATION_PACKET.md`](W1_IMPLEMENTATION_PACKET.md).

## W2 — Direct source spine

Source admission, identity/path history, registry/ownership, stable no-execute reads, residency-aware
revision CAS, text/code materialization and deterministic unitization. No index is required.

The daemon returns through a new package ticket and consumes its accepted W1 public handoff instead of
rereading the W1 implementation packet.

Bounded packet: [`W2_IMPLEMENTATION_PACKET.md`](W2_IMPLEMENTATION_PACKET.md).

W1 contributes to G1; accepted W2 closes G1.

## W3 — Qualified lexical index

Exact Qdrant process artifact and containment, data-plane capability/schema gate, lexical fixtures,
point identity/projection manifests, linearizable publication, epoch pins and exact retired-point
reclamation.

Bounded packet: [`W3_IMPLEMENTATION_PACKET.md`](W3_IMPLEMENTATION_PACKET.md).

## W4 — Baseline query product

Pre-candidate access, server-owned plans, bounded leg execution, exact source-backed validation, handle
state, compact cards, continuations and raw read/grep evaluation baseline.

Bounded packet: [`W4_IMPLEMENTATION_PACKET.md`](W4_IMPLEMENTATION_PACKET.md).

W3 contributes to G2; accepted W4 closes G2. The daemon receives separate W3 and W4 composition tickets
against the immediately previous accepted daemon handoff.

## W5 — Current workspace and code structure

Observation reconciliation, saved/unsaved overlays and qualified Rust structural enrichment.

Bounded packet: [`W5_IMPLEMENTATION_PACKET.md`](W5_IMPLEMENTATION_PACKET.md).

## W6 — Comparison and exact proof

Ambiguity-preserving subject resolution, descriptive cross-repository comparison and frozen-denominator
exact execution reports.

Bounded packet: [`W6_IMPLEMENTATION_PACKET.md`](W6_IMPLEMENTATION_PACKET.md).

W5 contributes to G3; accepted W6 closes G3. The daemon is re-ticketed separately for W5 and W6 and does
not accumulate W1–W4 implementation packets.

## W7 — Security and lifecycle hardening

Restrictive-revocation linearization, durable handles, CAS mark-and-sweep, purge receipts/tombstones,
restore quarantine and ordinary-reclaim/purge separation.

Bounded packet: [`W7_IMPLEMENTATION_PACKET.md`](W7_IMPLEMENTATION_PACKET.md).

W7 is not another central gate. It emits the exact `W7_LIFECYCLE` prerequisite receipt consumed by W8
and W9. Reused packages consume accepted earlier public handoffs plus only their package-local W7
hardening supplement; `search-revision-store` uses its accepted base `FUNCTIONS.md` and lifecycle/config
inputs because it has no invented separate W7 hardening file.

## W8 — Generic client edge

Full mutually authenticated binding/capability/evidence edge and standalone CLI. Optional ELIOT and
Research profiles remain leaf packages, disabled unless explicitly enabled and separately accepted.

Bounded packet: [`W8_IMPLEMENTATION_PACKET.md`](W8_IMPLEMENTATION_PACKET.md).

Reentries:

```text
search-provider-protocol  base FUNCTIONS.md + W8_HARDENING.md
eliot-searchd             accepted W7 daemon handoff + W8_INTEGRATION.md
eliot-search              accepted W1 CLI handoff + W8_CLIENT.md
```

W8 requires G3 plus `W7_LIFECYCLE` and closes G4.

## W9 — Product Pulse / Windows qualification

One exact Windows environment, immutable control corpus, pinned A/B baselines, accepted
DIRECT/LEXICAL/CODE candidate, pre-registered criteria, paired randomized execution, latency/resource/
fault/protocol/leakage evidence and independent Product Pulse verdict.

Bounded packet: [`W9_IMPLEMENTATION_PACKET.md`](W9_IMPLEMENTATION_PACKET.md). Machine packet:
[`../../swarm/w9-product-pulse.toml`](../../swarm/w9-product-pulse.toml).

Only `search-eval` is re-ticketed at W9. It consumes its accepted W4 public handoff plus accepted
lifecycle and W8/G4 evidence. The daemon is exercised through its accepted generic edge and is not
reopened merely to run Product Pulse.

Compilation, unit tests, screenshots, prose or post-hoc thresholds do not pass G5. Only exact `ACCEPTED`
plus independent reviewer receipt may be consumed by W10.

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
  -> search-eval paired baseline/candidate campaign
  -> removal/rollback proof
  -> independent G6 review

document:
  eliot-search-doc-worker sandbox/provider shell
  -> exact provider qualification
  -> candidate representation/projection generation
  -> search-eval paired baseline/candidate campaign
  -> removal/rollback proof
  -> independent G6 review

scale:
  measured one-shard bottleneck
  -> qdrant bridge topology qualification
  -> publication R0/catch-up/R1 migration
  -> guarded redb route switch
  -> route-pin drain and exact old-route reclaim
  -> search-eval paired benefit/nonregression/rollback campaign
  -> independent G6 review
```

`search-eval` reenters through `crates/search-eval/W10_OPTIONAL_EVALUATION.md`. It receives the accepted
W9 API and P15/G5 report/reviewer receipt, not W4/W9 implementation history. It constructs a candidate
G6 evidence bundle but cannot choose the candidate, mutate provider/route state or self-accept G6.

No candidate is selected in the scaffold. Feature presence, configuration, worker readiness, model name,
provider popularity or green unit tests cannot authorize serving. Optional failure must leave the
accepted P15 baseline available.

## Launch rule

For each package the orchestrator verifies package/function/stage/read-set/launch registry digests,
accepted direct and prior-stage API/configuration/evidence handoffs and exact artifact/profile receipts.
It creates one isolated package worktree, supplies only the current replacement context, rejects writes
outside package scope and merges accepted packages in dependency/stage order.

The current inventory is:

```text
11 stages
68 package-stage assignments
23 later-stage replacement contexts
16-file maximum static package context
```

Shared Cargo/dependency/profile/qualification/evidence changes belong to the integration owner. Package
writers cannot select external providers, enable optional profiles or self-accept a package, wave or
gate.
