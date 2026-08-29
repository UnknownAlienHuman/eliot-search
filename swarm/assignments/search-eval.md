# `search-eval` implementation packet

**Path:** `crates/search-eval`  
**Capability:** C29  
**Delivery:** W4 baseline / P08; acceptance W9 / P15  
**Gate:** BASELINE harness blocked until W4; acceptance verdict blocked until W8/P14 receipts  
**Trace:** S29.1, S30, S35-S37, P08, P15  
**Direct public handoffs:** `search-contracts`, `search-domain`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Own content-minimized telemetry, deterministic control-corpus fixtures, baseline comparison and the evidence-backed Product Pulse verdict.

## Owns

- control-corpus schema and fixture ownership
- A/B/C evaluation harness contracts
- quality, latency, resource, recovery and leakage metrics
- acceptance report structure and raw evidence receipts

## Must not own

- hidden training/learning
- source/query/unsaved content in default telemetry
- self-declared green acceptance from unit tests
- changing production ranking to fit the fixture

## Logical primitives

- ControlCorpusCase, OracleExpectation, EvaluationRun, BaselineDescriptor, MetricObservation, LeakageAudit, RecoveryMatrix, ProductPulseReport, AcceptanceVerdict

## Logical operations

1. `load_control_corpus(manifest) -> Result<ControlCorpus, EvalError>`
2. `run_case(provider, case, budget) -> CaseResult`
3. `compare_baselines(a, b, c) -> ComparisonReport`
4. `audit_content_minimization(events) -> LeakageAudit`
5. `aggregate_product_pulse(runs) -> ProductPulseReport`
6. `decide_acceptance(report, criteria) -> AcceptanceVerdict`

## Required invariants

- raw evidence and failed/skipped checks remain visible
- Product Pulse is the only P15 acceptance decision
- semantic/document depth cannot begin before accepted verdict
- telemetry contains IDs/reasons/counts/durations but no source/query/secret/absolute-path content
- fork/mirror and access/stale cases are represented

## Typed failure surface

- `EVALUATION_FIXTURE_INVALID`
- `EVALUATION_INCOMPLETE`
- `LEAKAGE_DETECTED`
- `PRODUCT_NOT_ACCEPTED`
- `QUALIFICATION_ENVIRONMENT_UNAVAILABLE`

## Exit tests / evidence

- `control_corpus_manifest_validation`
- `raw_grep_read_baseline`
- `content_minimized_telemetry_audit`
- `publication_fault_matrix_ingestion`
- `access_leakage_negative_cases`
- `verdict_rejects_missing_raw_evidence`

## Suggested internal modules

```text
search-eval/src/
  corpus.rs
  oracle.rs
  runner.rs
  baseline.rs
  metric.rs
  leakage.rs
  recovery.rs
  report.rs
  verdict.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 7,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Keep measurement/report/verdict together for auditability. Platform drivers may be external test tooling but cannot own the verdict.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.
