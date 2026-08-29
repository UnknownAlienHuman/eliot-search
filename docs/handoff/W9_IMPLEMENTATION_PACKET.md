# W9 Product Pulse implementation packet

**Stage / wave / gate:** P15 / W9 / G5  
**Status:** `BLOCKED`; this packet does not authorize implementation or acceptance.  
**Package writer:** `search-eval` only.  
**Execution owner:** integration owner.  
**Acceptance owner:** independent reviewer.

## Purpose

W9 converts the completed DIRECT/LEXICAL/CODE baseline into one reproducible Product Pulse decision on
an exact Windows x64 environment. It measures quality, efficiency, latency, resources, recovery,
protocol flow control and content/security leakage without changing production behavior.

## Package packet

| Owner | Bounded files | Write scope |
|---|---|---|
| `search-eval` writer | assignment, package `AGENTS.md`/`FUNCTIONS.md`, W9 cross-contract, corpus/metric/probe/gate-map schemas, W9 settings, accepted dependency handoffs | `crates/search-eval/**` |
| integration owner | accepted W0–W8 receipts, Windows/artifact/run identities, cross-package drivers and immutable raw evidence | integration-owned dev/test and evidence paths |
| independent reviewer | pre-registered criteria review, reproduction sample and final verdict receipt | reviewer/evidence receipt path |

The package writer cannot edit another package's fixture, execute cross-package lifecycle mutations,
populate shared evidence slots or accept G5.

## Dependency and execution order

```text
accepted G0–G4 receipts
+ accepted W7 security/lifecycle receipt
+ exact package API/handoff digest set
+ exact qualified Windows/artifact set
        ↓
materialize and independently review control corpus/oracles
        ↓
select exact A/B/C baselines and freeze quality/resource policy
        ↓
freeze Windows environment and run manifest
        ↓
paired randomized cold/warm A/B/C execution
        ↓
fault/recovery + protocol stress + admission/leakage audits
        ↓
search-eval deterministic report and hard-blocker classification
        ↓
independent reproduction/review
        ↓
ACCEPTED | REJECTED | blocked/incomplete state
```

Candidate C results may not be observed before corpus, baseline, environment, metrics and acceptance
policy are frozen.

## Required inputs

- `docs/evaluation/W9_PRODUCT_PULSE_CONTRACTS_1.0.md`;
- `qualification/product-pulse/W9_QUALIFICATION.md`;
- `qualification/product-pulse/{baseline,corpus,metrics,probes,gate-map,fixture-owners}.toml`;
- `config/w9-product-pulse.toml` and settings contract;
- `swarm/w9-product-pulse.toml`;
- exact accepted dependency/API/evidence refs supplied by the ticket.

Ordinary package agents do not reload the architecture master.

## Hard invariants

- A/B/C use one declared scope, view, network policy and case snapshot;
- oracle labels and policy thresholds never enter production or baseline drivers;
- cold, warm, preparation, background-pressure and recovery lanes remain distinct;
- failed, skipped, cancelled, timed-out and unavailable observations remain visible;
- stale/access/secret/content leakage and false complete negatives have zero tolerance;
- hard correctness/security blockers cannot be averaged away;
- every PASS has immutable raw output and independent review;
- producer cannot be the accepting reviewer;
- optional profiles remain disabled during baseline acceptance;
- only an exact `ACCEPTED` P15 receipt may appear in a W10 ticket.

## Required implementation seams

The `search-eval` agent implements the operations in package `FUNCTIONS.md` for:

- corpus, metric, policy, baseline, environment and run validation;
- deterministic seeded case-block planning;
- dev/test evidence-driver abstraction;
- case evidence validation and reproducibility;
- metric scoring, coherent aggregation and paired A/B/C comparison;
- SLO and Windows resource reports;
- source-admission/content-minimization, fault and protocol audits;
- Product Pulse report, hard-blocker set, closed verdict and immutable receipt.

It does not implement a second provider protocol, Qdrant/redb/CAS adapter, source reader, scheduler or
production telemetry sink.

## Evidence handoff

A package handoff contains public API digest, deterministic schema/aggregation tests, dev/test fake
drivers and exact command output. It explicitly labels unavailable cross-package/runtime evidence.

An integration evidence record additionally contains exact commit/API/configuration/corpus/environment/
artifact identities, command/fixture digest, raw-output ref/digest and independent reviewer receipt.

## Hard stop conditions

- missing G4 or W7 receipt;
- any `UNSELECTED`/`UNMATERIALIZED` run identity;
- fewer than eight proven independent reference lineages;
- mutable baseline/corpus/policy/environment;
- post-hoc criteria, weights, cases or retry-only-success selection;
- mixed scope/view/cache/environment samples;
- any hard safety/correctness blocker;
- missing raw evidence or independent reviewer;
- optional profile enabled in the baseline;
- product or optional-depth authorization through configuration.

## Current state

```text
search-eval W9 implementation: BLOCKED
control corpus: UNMATERIALIZED
baseline A/B: UNSELECTED
candidate C runtime: UNAVAILABLE
Windows profile: UNSELECTED
quality acceptance policy: UNSELECTED
mandatory probes: 60 UNAVAILABLE
Product Pulse: NOT ACCEPTED
optional depth: BLOCKED
```
