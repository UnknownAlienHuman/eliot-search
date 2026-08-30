# W10 optional-depth implementation packet

**Stage / wave / gate:** P16-P18 / W10 / G6  
**Status:** `BLOCKED`; no provider or topology selected.  
**Prerequisite:** exact independently accepted P15 Product Pulse plus one candidate-specific ADR and
integration ticket.

## Candidate rule

One ticket selects exactly one candidate class/profile:

```text
model:<profile digest>
document:<profile digest>
scale:<profile digest>
```

Acceptance or implementation of one candidate does not authorize another. A changed model, tokenizer,
runtime, quantization, document engine, limit/coordinate policy or scale topology is a new candidate.

## Package packets

| Package | Stage | Bounded packet | Write scope |
|---|---:|---|---|
| `search-model-provider` | P16 | `crates/search-model-provider/FUNCTIONS.md` | package only |
| `eliot-search-model-worker` | P16 | `bins/eliot-search-model-worker/FUNCTIONS.md` | package only |
| `eliot-search-doc-worker` | P17 | `bins/eliot-search-doc-worker/FUNCTIONS.md` | package only |
| `eliot-searchd` | P16-P18 | `bins/eliot-searchd/W10_INTEGRATION.md` | package only, integration ticket |
| `search-qdrant-bridge` | P18 | package `FUNCTIONS.md` + `P18_SCALE.md` | package only |
| `search-publication` | P16-P18 migration | package `FUNCTIONS.md` + `P18_SCALE.md` | package only |
| `search-epoch-pins` | P16-P18 migration | package `FUNCTIONS.md` + `P18_SCALE.md` | package only |
| `search-index-reclaimer` | P16-P18 migration | package `FUNCTIONS.md` + `P18_SCALE.md` | package only |
| `search-eval` | P16-P18 candidate evidence | accepted W9 `FUNCTIONS.md` + `crates/search-eval/W10_OPTIONAL_EVALUATION.md` | package only |

Shared profile selection, external dependency/Cargo changes, configuration registry, qualification
evidence, daemon feature wiring, fixtures and gate receipts remain integration-owner changes.

## Read set

`swarm/w10-optional-depth.toml` is the candidate machine packet. `swarm/stage-readsets.toml` replaces
previous-stage documents for reused packages with exact accepted public handoffs and one current-stage
delta. A package writer reads its local instructions/functions, current candidate/profile packet,
current W10 contract/settings/qualification and accepted direct/prior-stage handoffs only. Architecture
access is exception-only.

The W10 `search-eval` writer does not reread the W4 or W9 implementation packets. It receives the exact
accepted W9/P15 package/API/report/reviewer receipts, `W10_OPTIONAL_EVALUATION.md`, current candidate
qualification registries and candidate-specific fixture/operation receipts.

## Model order

```text
search-model-provider contract implementation and pure fakes
-> model worker private protocol/resource shell
-> exact artifact/runtime/model/tokenizer qualification
-> rerank-only or dense/multivector candidate plan
-> candidate generation when persistent vectors exist
-> search-eval paired baseline/candidate campaign
-> removal/rollback proof
-> G6 independent review
-> staged daemon activation
```

Rerank never widens the candidate set. Dense/multivector output remains nomination only and always
requires exact source validation. Model failure cannot disable P15 baseline.

## Document order

```text
document worker no-execute/resource shell and fake provider
-> exact provider/runtime/Windows/license selection by ADR
-> malformed/fuzz/archive/page/object/remote-resource qualification
-> coordinate/loss-map and assurance goldens
-> candidate representation/projection generation
-> search-eval paired baseline/candidate campaign
-> removal/rollback proof
-> G6 independent review
-> staged daemon activation
```

The worker has no store/index/client access and no script/macro/network/remote-resource path.

## Scale order

```text
accepted measured one-shard bottleneck
-> dedicated topology/scoring/migration ADR
-> exact server/client/topology qualification
-> candidate generation base at R0
-> ordered change-log catch-up
-> final barrier at R1
-> exact equivalence and fault validation
-> guarded redb route switch
-> old-route pin drain and exact reclaim
-> search-eval paired benefit/nonregression/rollback campaign
-> G6 independent review
```

A Qdrant alias is never the route commit. Active schema/topology is never changed in place.

## Candidate evaluation boundary

`search-eval` owns candidate-specific measurement meaning only:

- exact accepted P15 baseline plus one candidate identity;
- pre-registered paired baseline/candidate task, metric, denominator and threshold policy;
- candidate fixture extension with independent oracle labels;
- separate cold/warm/preparation/migration/recovery lanes;
- incremental quality and resource cost;
- access/currentness/content/authority noninterference;
- worker/provider/migration/removal fault matrix;
- complete optional removal plus accepted P15 regression;
- immutable five-record G6 evidence candidate.

It does not select or execute a provider directly, open Search/provider/Qdrant stores, control daemon
activation, mutate routes, choose post-hoc thresholds or self-accept G6. Operation receipts are consumed
through exact package/public evidence contracts.

## Required G6 evidence

Every selected candidate supplies all five central evidence IDs:

1. `dedicated_optional_profile_adr`
2. `exact_provider_artifact_qualification`
3. `measured_material_benefit`
4. `removal_or_uninstall_fallback`
5. `migration_and_rollback_when_applicable`

A rerank-only candidate may satisfy item 5 with an independently reviewed no-persistent-state receipt;
it may not omit the evidence record.

## Package handoff requirements

Each package handoff includes:

- exact base/commit and public API digest;
- implemented operations and package-local state owner;
- deterministic/property/negative/fault tests;
- cancellation/deadline/resource and content-minimization evidence;
- exact dependency/artifact/feature/config requirements, still unavailable where unselected;
- removal/degradation behavior;
- line count and split review status;
- explicit unavailable runtime/G6 evidence.

The `search-eval` reentry additionally reports exact accepted W9/P15 baseline identity, candidate campaign
schema/metric digest and proves that prior package behavior and cumulative line budget were not reset.
Package writers cannot mark a shared qualification probe PASS or accept their own gate.

## Hard stop conditions

- P15 report/reviewer receipt missing or mismatched;
- provider/topology/artifact/runtime/ADR/feature not exact;
- one ticket attempts multiple candidates;
- W10 evaluator rereads/reinterprets W9 implementation history instead of consuming the accepted handoff;
- network, auto-download/update, training/learning or persistent content cache;
- model output treated as evidence/answer/exact proof or rerank widens candidates;
- document worker can execute code, follow remote resources or escape temp/root policy;
- missing coordinate/loss map or assurance overclaim;
- persistent schema/profile changed in place;
- optional failure breaks P15 baseline;
- material benefit/cost/safety/removal evidence absent;
- scale without measured bottleneck or incomplete migration/pin/rollback matrix;
- configuration/worker readiness claims activation;
- automatic provider switching or package/evaluator self-acceptance.

## Current state

```text
accepted P15: UNSELECTED
model candidate: DISABLED
model worker: ABSENT
document candidate: DISABLED
document worker: ABSENT
scale candidate: DISABLED
search-eval candidate campaign: BLOCKED
G6 probe templates: 45 DISABLED
G6 accepted candidates: NONE
baseline P15 behavior: authoritative
```
