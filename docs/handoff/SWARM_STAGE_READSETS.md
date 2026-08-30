# Swarm stage-specific read sets

This document defines how the integration owner constructs the smallest correct context for one package
agent at one implementation stage.

Machine authorities:

```text
swarm/crates.toml             package path, dependencies, earliest wave and assignment
swarm/function-packets.toml   primary function/contract packet and package write scope
swarm/stages.toml             W0–W10 package sets, shared stage context and receipt/gate order
swarm/stage-readsets.toml     later-stage context replacements for reused packages
swarm/launch-state.toml       current implementation authorization only
```

A stage or read-set entry never authorizes implementation. The integration-owner ticket still requires
an exact base commit, active writer lease and accepted immutable dependency/wave/gate receipts.

## 1. Context assembly

For package `P` at stage `W`, the orchestrator assembles exactly:

1. root `AGENTS.md` and the nearest package/family `AGENTS.md`;
2. `docs/handoff/AUTHORITY_MAP.md` and `swarm/ASSIGNMENT_PROTOCOL.md`;
3. the exact `swarm/crates.toml` entry for `P`;
4. the exact `swarm/function-packets.toml` entry for `P`;
5. the exact `swarm/stages.toml` entry for `W`;
6. the package assignment and primary function/contract packet;
7. the stage `shared_read_set`;
8. when `P` is reused after its earliest wave, the one matching
   `swarm/stage-readsets.toml` override;
9. exact accepted direct-dependency and prior-stage public handoff receipts named by the ticket;
10. named fixture references owned by the applicable qualification registry.

The exact registry entries are injected as bounded snippets rather than mounting whole machine files.
Stage `machine_packet` files remain integration-owner registries and are not ordinary agent context.
No other repository documentation or package implementation is mounted by default.

## 2. Earliest-wave packages

A package at its earliest wave needs no override. Its context is:

```text
root/package instructions
+ package/dependency registry entry
+ assignment and primary function packet
+ current stage shared_read_set
+ package configuration/qualification files explicitly named by the stage ticket
+ exact accepted handoff/fixture refs
```

This avoids duplicating 45 nearly identical base read-set records.

## 3. Reused packages

Every package assignment after the package's earliest wave receives a replacement context, not an
accumulated history.

The override has these mandatory semantics:

```text
replace_previous_stage_context = true
accepted_prior_stage_handoff_only = true
dependency_implementation_reads_allowed = false
shared_registry_edits_allowed = false
```

The override names both:

```text
base_stage   first stage that established the package API
prior_stage  immediate accepted package stage whose handoff the new writer consumes
```

The base and immediate-prior implementation packets are explicitly forbidden. For packages with several
progressive stages, all earlier stage packets are forbidden. The later agent consumes immutable public
API/configuration/evidence handoffs instead.

There are currently **twenty-three later-stage assignments** across thirteen reused packages:

```text
W2
  eliot-searchd

W3
  eliot-searchd

W4
  eliot-searchd

W5
  eliot-searchd

W6
  eliot-searchd

W7
  eliot-searchd
  search-revision-store
  search-access
  search-handles
  search-continuation
  search-candidate-validator
  search-publication
  search-index-reclaimer

W8
  search-provider-protocol
  eliot-searchd
  eliot-search

W9
  search-eval

W10
  eliot-searchd
  search-qdrant-bridge
  search-publication
  search-epoch-pins
  search-index-reclaimer
  search-eval
```

The six W2–W7 daemon overrides are required by the Cargo feature ladder:

```text
wave1-shell
→ wave2-source
→ wave3-index
→ wave4-query
→ wave5-current
→ wave6-proof
→ wave7-lifecycle
```

The W1 writer cannot implement concrete later composition against APIs that do not yet have accepted
handoffs. One package is still owned by one writer at a time; `eliot-searchd` receives a new isolated
package ticket after each accepted wave.

`search-retention`, optional workers and optional leaf adapters first appear in their current stage and
therefore use normal earliest-wave assembly.

## 4. Exact reentry examples

### `eliot-searchd`

```text
W1  base daemon FUNCTIONS.md + W1 shell packet
W2  base FUNCTIONS.md + W2 source-spine packet + accepted daemon W1 handoff
W3  base FUNCTIONS.md + W3 index packet + accepted daemon W2 handoff
W4  base FUNCTIONS.md + W4 query packet + accepted daemon W3 handoff
W5  base FUNCTIONS.md + W5 currentness packet + accepted daemon W4 handoff
W6  base FUNCTIONS.md + W6 proof packet + accepted daemon W5 handoff
W7  base FUNCTIONS.md + W7 lifecycle packet + accepted daemon W6 handoff
W8  base FUNCTIONS.md + W8_INTEGRATION.md + accepted daemon W7/lifecycle handoffs
W10 base FUNCTIONS.md + W10_INTEGRATION.md + accepted daemon W8/P15/candidate handoffs
```

Each daemon ticket receives only the current stage packet. For example, W7 does not receive W1–W6
implementation packets; their accepted daemon handoff replaces them.

### `eliot-search`

```text
W1  base CLI FUNCTIONS.md and shell handoff
W8  same base FUNCTIONS.md
    + bins/eliot-search/W8_CLIENT.md
    + W8 generic-edge shared files
    + accepted CLI W1 and protocol W8 handoffs
```

The W8 client writer does not reload the W1 implementation packet. It cannot infer endpoint/pairing/
capability behavior from protocol internals.

### `search-publication`

At W3:

```text
base FUNCTIONS.md
+ W3 shared packet and Qdrant qualification
+ exact accepted W2 handoffs
```

At W7:

```text
same base FUNCTIONS.md
+ W7_HARDENING.md
+ W7 lifecycle shared packet/settings/qualification
+ accepted search-publication W3 API receipt
+ accepted G3 receipt
```

At W10:

```text
same base FUNCTIONS.md
+ P18_SCALE.md
+ W10 optional-depth shared packet/settings/qualification
+ accepted search-publication W7 API receipt
+ accepted P15/G5 receipt
+ candidate-specific G6 inputs
```

The W10 writer does not reload W3 or W7 packets. Their accepted public handoffs replace those documents.

### `search-eval`

```text
W4  baseline evaluation/query qualification context

W9  base FUNCTIONS.md
    + Product Pulse contract/corpus/metrics/probes
    + accepted search-eval W4 API, G4 and lifecycle receipts

W10 base FUNCTIONS.md
    + W10_OPTIONAL_EVALUATION.md
    + optional-depth baseline/probes/gate-map/fixture-owner registries
    + accepted search-eval W9 API and accepted P15/G5 receipt
    + one candidate-specific G6 input set
```

The W9 agent receives no W4 stage packet and cannot execute or edit cross-package fault fixtures. The
W10 agent receives neither W4 nor W9 implementation packets; it evaluates exactly one candidate against
the immutable accepted P15 baseline and cannot self-accept G6.

## 5. Context ceilings

The static package context is capped at sixteen files, including:

```text
root AGENTS.md
package AGENTS.md
AUTHORITY_MAP.md
ASSIGNMENT_PROTOCOL.md
assignment
primary function/contract
current stage shared files
stage-specific supplements/additional files
```

The current largest static packet is W9 `search-eval` at exactly sixteen files. W10 `search-eval` uses
fifteen. Registry entries are injected as bounded snippets and do not mount entire registries into the
agent context.

A ticket may add at most sixteen immutable handoff receipts and sixteen named fixture references. These
are accepted public artifacts/references, not another package's source tree or implementation history.

If more context is required, the integration owner must split the task, add a bounded public contract or
change the registry through a reviewed integration PR. The writer may not widen its own context.

## 6. Ticket requirements

A stage-specific assignment ticket contains:

```yaml
stage:
package:
base_commit:
writer_lease:
write_scope:
crates_registry_digest:
function_registry_digest:
stage_registry_digest:
stage_readset_registry_digest:
launch_state_digest:
assignment_and_primary_packet_digests:
stage_shared_file_digests: []
override_file_digests: []
accepted_direct_handoffs: []
accepted_prior_stage_handoffs: []
fixture_refs: []
required_commands: []
explicit_unavailable_checks: []
```

Every digest is immutable for the ticket. A mutable branch or PR head is not a dependency handoff.

## 7. Write and authority boundaries

- one active writer edits one package and only the exact function-registry write scope;
- a later stage creates a new package lease/worktree; it does not permit concurrent daemon/evaluator
  writers;
- stage overrides never widen write scope;
- package agents cannot edit `swarm/`, root Cargo/lock/toolchain/CI, architecture, central config,
  qualification/evidence registries or another package;
- shared registry, artifact/provider selection and gate/wave receipts are integration-owner work;
- a later-stage writer cannot redefine a previously accepted base API privately;
- architecture access remains exception-only after a concrete contract challenge;
- package or workflow success cannot self-accept a gate.

## 8. Stage and gate chain

`swarm/stages.toml` separates stage completion receipts from central gates:

```text
W0 closes G0
W1 contributes to G1
W2 closes G1
W3 contributes to G2
W4 closes G2
W5 contributes to G3
W6 closes G3
W7 emits the separate W7_LIFECYCLE prerequisite receipt
W8 closes G4
W9 closes G5 after Product Pulse review
W10 closes candidate-specific G6
```

The daemon's progressive package handoff at a stage is part of that stage receipt; it is not an extra
central gate. W7 lifecycle hardening is not mistaken for G4 or product acceptance.

## 9. Hard stops

Stop ticket generation when:

- package/stage/wave/path/write-scope differs across registries;
- a later package assignment lacks exactly one override;
- an earliest-wave package has an unnecessary override;
- `prior_stage` is not the package's immediate previous stage;
- a later override includes a forbidden prior-stage packet;
- prior package implementation is supplied instead of an accepted public handoff;
- static context exceeds sixteen files;
- a supplement/additional file is missing or belongs to the architecture master;
- required gate/receipt is absent or mismatched;
- launch state does not authorize the package;
- ticket attempts to edit a shared registry or another package;
- provider/artifact/profile remains unselected where the task requires it.

## 10. Current disposition

```text
stages:                         11 (W0–W10)
stage-package assignments:      68
later-stage replacement sets:   23
unique reused packages:         13
maximum static files:           16 / 16
current stage:                  W0 / P00
currently authorized:           search-contracts only
runtime implementation:         absent
accepted wave receipts:         absent
```
