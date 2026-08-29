# Agent contract — search-eval

You own only `crates/search-eval/`. Do not edit another package, the root workspace, shared contracts,
qualification evidence or architecture. A missing field uses the contract-change process; do not patch
another owner's fixture or weaken an invariant.

## Bounded read set

For W4 baseline work read the package assignment, `FUNCTIONS.md`, observability section and accepted
direct handoffs. For W9/P15 work additionally read only:

- `docs/evaluation/W9_PRODUCT_PULSE_CONTRACTS_1.0.md`;
- `qualification/product-pulse/{baseline,corpus,metrics,probes,gate-map}.toml`;
- `config/w9-product-pulse.toml`;
- `swarm/w9-product-pulse.toml`;
- accepted immutable fixture/API/evidence references supplied by the integration ticket.

The Architecture 8.4 master is exception-only. `swarm/launch-state.toml` remains the only launch
authority.

## Mission

Own content-minimized observability, deterministic evaluation schemas and the evidence-backed Product
Pulse verdict without becoming a production dependency, training pipeline or gate self-approver.

## Ownership

- corpus, baseline, run, metric and evidence schemas;
- deterministic case/block aggregation and A/B/C comparison;
- latency/resource/recovery/protocol/security reports;
- content-minimization and source-admission leakage audits;
- Product Pulse report and closed verdict construction.

## Forbidden ownership

- production ranking/query/source/index behavior;
- raw source/query/unsaved/secret/token/path content in ordinary reports or telemetry;
- hidden learning, training, query-specific tuning or oracle feedback;
- changing another package's fault fixture;
- executing stores or Qdrant through a production dependency;
- selecting thresholds after candidate results;
- self-reviewing or self-accepting G5;
- authorizing optional depth without an exact accepted P15 receipt.

## Allowed dependencies

`search-contracts`, `search-domain`, `search-config`. Additional dependencies require an explicit
boundary review. Integration execution uses a dev/test driver over accepted provider/public interfaces;
production packages never depend on `search-eval`.

## Required operation surface

`FUNCTIONS.md` is authoritative. It includes corpus/policy validation, frozen run identity, baseline
validation, deterministic execution blocks, evidence ingestion/reproduction, scoring/aggregation,
A/B/C comparison, SLO/resource reports, leakage/admission/fault/protocol audits, Product Pulse report,
hard blockers, verdict and immutable receipt.

## Invariants

- identical declared scope and immutable case snapshot across A/B/C;
- oracle and criteria hidden from production/baselines;
- cold/warm/preparation/recovery lanes remain distinct;
- every failed/skipped/cancelled/timed-out/unavailable observation remains visible;
- safety/correctness failures cannot be averaged away;
- environment/corpus/artifact/configuration drift creates a new run;
- producer and accepting reviewer are different identities;
- only `ACCEPTED` may unlock W10.

## Size and split guard

- normal target: at most 7,500 hand-written `src/` lines;
- split review before 8,500 total hand-written lines;
- hard stop at 10,000 including package-local tests;
- keep schemas/aggregation/verdict together for auditability;
- platform execution drivers may be dev/test tooling, but they cannot own verdict meaning or become a
  production dependency.

## Handoff

Report exact commands, immutable raw evidence references, failures/unavailable checks and public API
digest. Compilation and unit tests are structural evidence only; they never claim G5 acceptance.
