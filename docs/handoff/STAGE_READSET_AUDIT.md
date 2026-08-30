# Stage-specific Swarm read-set audit

**Status:** structural contract audit only.  
**Current launch:** P00 / W0; only `search-contracts` authorized.  
**Architecture access:** exception-only.

## Closure result

The implementation scaffold resolves to:

```text
45 earliest-wave base package assignments
23 later-stage package re-entry assignments
68 total package-stage assignments
13 unique reused packages
11 stages: W0 through W10
maximum static package context: 16 files
```

Every Cargo package appears exactly once at its registered earliest wave. A later appearance is valid
only when `swarm/stage-readsets.toml` contains one exact replacement override for that `(stage, package)`.
An earliest-wave assignment must not have an override.

## Re-entry matrix

| Stage | Re-entry packages | Purpose |
|---|---|---|
| W2 | `eliot-searchd` | compose accepted source-spine ports without replaying W1 internals |
| W3 | `eliot-searchd` | compose qualified lexical/Qdrant profile |
| W4 | `eliot-searchd` | compose accepted query handlers and capability snapshot |
| W5 | `eliot-searchd` | compose current-workspace/reconciliation/overlay behavior |
| W6 | `eliot-searchd` | compose resolution/comparison/exact-proof behavior |
| W7 | `eliot-searchd`, `search-revision-store`, `search-access`, `search-candidate-validator`, `search-handles`, `search-continuation`, `search-publication`, `search-index-reclaimer` | lifecycle/security hardening without reopening previous implementation packets |
| W8 | `search-provider-protocol`, `eliot-searchd`, `eliot-search` | authenticated generic client edge and standalone CLI reentry |
| W9 | `search-eval` | Product Pulse campaign/report meaning from accepted W4/lifecycle/G4 handoffs |
| W10 | `eliot-searchd`, `search-qdrant-bridge`, `search-publication`, `search-epoch-pins`, `search-index-reclaimer`, `search-eval` | optional activation, P18 scale migration and candidate-specific evaluation |

The daemon's W2–W7 appearances are sequential feature-ladder package tickets, not concurrent agents.
W9 does **not** reopen the daemon: Product Pulse executes through the accepted generic edge and consumes
accepted daemon/build/runtime evidence. W10 reopens `search-eval` only for one candidate-specific paired
campaign after an accepted P15 handoff.

## Replacement semantics

Each of the twenty-three reentry assignments has:

```text
base_stage
prior_stage
replace_previous_stage_context = true
accepted_prior_stage_handoff_only = true
forbidden_prior_stage_packets
required_prior_handoffs
dependency_implementation_reads_allowed = false
shared_registry_edits_allowed = false
```

The previous implementation packet is never accumulated with the current stage packet. Immutable public
API/configuration/evidence handoffs replace implementation history.

Examples:

### `eliot-search`

```text
W1 accepted CLI handoff
+ W8 stage shared packet
+ bins/eliot-search/W8_CLIENT.md
+ accepted W8 protocol handoff
```

The W1 implementation packet is forbidden. The W8 writer owns only endpoint/pairing/session/request/
rendering/exit behavior and cannot inspect protocol internals or stores.

### `search-publication`

```text
W3 base: FUNCTIONS.md + W3 index context
W7 reentry: FUNCTIONS.md + W7_HARDENING.md + accepted W3 handoff
W10 reentry: FUNCTIONS.md + P18_SCALE.md + accepted W7/P15/candidate handoffs
```

The W10 agent does not reread W3 or W7 implementation packets.

### `search-eval`

```text
W4 base: query/evaluation primitives
W9 reentry: Product Pulse context + accepted W4/lifecycle/G4 handoffs
W10 reentry: W10_OPTIONAL_EVALUATION.md + candidate registries + accepted W9/P15 handoff
```

The W10 evaluator cannot reuse mutable W9 history, alter the accepted baseline or self-accept G6.

## Context accounting

The validator recomputes static context as:

```text
6 base files
  root AGENTS
  package AGENTS
  AUTHORITY_MAP
  ASSIGNMENT_PROTOCOL
  assignment
  primary function/contract

+ unique current-stage shared_read_set
+ exact supplements
+ exact additional_files
```

The ceiling is sixteen files. W9 `search-eval` uses sixteen; W10 `search-eval` uses fifteen; W8
`eliot-search` uses eleven.

Registry snippets, accepted handoff receipts and fixture references are injected separately under their
own sixteen-item ceilings. Entire registries, architecture documents and dependency source trees are not
mounted.

## Stage/gate ordering

```text
W0 closes G0
W1 contributes to G1; W2 closes G1
W3 contributes to G2; W4 closes G2
W5 contributes to G3; W6 closes G3
W7 emits separate W7_LIFECYCLE receipt
W8 closes G4 and requires G3 + W7_LIFECYCLE
W9 closes G5 and requires G4 + W7_LIFECYCLE
W10 closes candidate-specific G6 and requires accepted P15/G5
```

A lifecycle receipt is not silently treated as a central gate. Package/stage presence is not a gate
receipt.

## Ownership and write isolation

The stage registry changes context only. It does not change ownership:

- package path/dependencies/earliest wave: `swarm/crates.toml`;
- primary behavior/write scope: `swarm/function-packets.toml`;
- stage membership and ordering: `swarm/stages.toml`;
- replacement context/prior handoffs: `swarm/stage-readsets.toml`;
- current permission: `swarm/launch-state.toml`.

One writer still edits one package-only worktree. Shared registries, Cargo/lockfile/toolchain, architecture,
central qualification/evidence and cross-package fixtures remain integration-owned.

## Structural enforcement

`tools/validate-stage-readsets.ps1` rejects:

- missing/duplicate W0–W10 stage or an assignment count other than 68;
- package absent or first appearing outside its earliest wave;
- any later assignment without exactly one override;
- any earliest-wave assignment with an override;
- wrong `base_stage` or immediate `prior_stage`;
- missing/extra supplement or package-specific additional file;
- prior implementation packet present in active context;
- architecture/dependency implementation reads;
- static context above sixteen files;
- write-scope mismatch against the function registry;
- missing W8 client or W10 evaluator delta operations;
- launch advancement beyond P00/W0;
- automatic or write-enabled GitHub Actions workflow.

## Non-claims

This audit does not prove:

- any package implementation;
- Rust toolchain/dependency/lockfile acceptance;
- Windows ownership, ACL, secret, process or filesystem behavior;
- redb/CAS/Qdrant correctness;
- source/currentness/query/lifecycle runtime behavior;
- Product Pulse or optional-provider acceptance;
- that any W1+ assignment may start now.

It proves only that the future Swarm can receive deterministic, narrow package-stage contexts without
repeatedly reading the architecture, historical implementation packets or unrelated package internals.
