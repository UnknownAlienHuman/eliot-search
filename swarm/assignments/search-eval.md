# `search-eval` implementation packet

**Path:** `crates/search-eval`  
**Capability:** C29  
**Delivery:** W4/P08 baseline schemas; W9/P15 Product Pulse  
**Gate:** baseline harness blocked until W4; W9 blocked until accepted G4 plus lifecycle/security receipts  
**Trace:** S29.1, S30, S35–S37, P08, P15  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-config`

Apply `../ASSIGNMENT_PROTOCOL.md`. `FUNCTIONS.md` owns operation semantics. W9 ordinary read scope is
listed in `swarm/w9-product-pulse.toml`; architecture access is exception-only.

## Mission

Own deterministic, content-minimized evaluation meaning and construct an evidence-backed Product Pulse
verdict without becoming a production dependency, training pipeline or gate self-approver.

## Owns

- control-corpus, oracle, baseline, environment, metric and frozen-run schemas;
- paired randomized A/B/C comparison semantics;
- quality/efficiency, latency/resource, fault/recovery and protocol-stress reports;
- source-admission and content-minimization leakage audits;
- hard-blocker classification;
- Product Pulse report, closed verdict and immutable receipt.

## Must not own

- production ranking/query/source/index/lifecycle behavior;
- another package's fixture semantics or cross-package fault mutation;
- raw source/query/unsaved/secret/token/path content in ordinary telemetry/reports;
- hidden learning, training, oracle feedback or query-specific tuning;
- post-hoc thresholds, case removal or retry-until-pass selection;
- direct Qdrant/redb/CAS/source-store access in production;
- self-review or self-acceptance;
- optional-depth authorization without an exact accepted P15 receipt.

## Required operations

See package `FUNCTIONS.md` for:

1. control-corpus/metric/policy/baseline/run validation;
2. deterministic case-block planning and dev/test execution-driver seam;
3. evidence validation/ingestion/reproduction;
4. case scoring and coherent aggregation;
5. paired A/B/C and SLO/resource reports;
6. leakage/admission/fault/protocol audits;
7. Product Pulse report, hard blockers, verdict and append-only receipt.

## Invariants

- identical declared scope/view/case snapshot across A/B/C;
- oracle and criteria hidden from production and baseline drivers;
- cold/warm/preparation/recovery observations never mix;
- every failed/skipped/cancelled/timed-out/unavailable observation remains visible;
- hard safety/correctness failures cannot be averaged away;
- all identities are exact and immutable;
- producer and accepting reviewer differ;
- only `ACCEPTED` unlocks W10.

## Integration boundary

The package writer implements schemas, pure aggregation, fake/dev-test driver traits and tests only
inside the package. The integration owner executes the accepted product on Windows, publishes immutable
raw evidence and assembles G5 references. The independent reviewer freezes criteria before candidate
results and accepts or rejects the final report.

## Exit tests / evidence

- complete S35 case registry and eight-lineage minimum;
- exact run/environment/artifact/configuration identities;
- deterministic seeded paired ordering;
- no hidden failures or post-hoc criteria;
- zero-tolerance canary audits;
- fault and protocol matrices cannot pass incomplete;
- Architecture SLO timer and percentile fixtures;
- DOMINATES/COMPLEMENTS policy goldens;
- hard blocker defeats aggregate quality;
- producer cannot self-review;
- report/receipt redaction and append-only identity;
- production dependency-direction guard.

## Size / split

- target `src/` ≤7,500 hand-written lines;
- split review before 8,500 total hand-written lines;
- hard stop at 10,000 including local tests;
- keep schemas/aggregation/verdict together for auditability;
- platform execution drivers remain dev/test tooling and cannot own verdict meaning.
