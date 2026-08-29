# Agent contract — search-eval

You own only `crates/search-eval/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S29.1, S30.2, S35-S37, P08, P15.

## Mission

Own content-minimized observability, control-corpus evaluation and property/fault evidence without becoming a training pipeline.

## Ownership

- opaque metrics and operation traces
- control-corpus harness and baseline adapters
- property/fault fixture orchestration
- latency/resource/security acceptance reports
- privacy leakage assertions

## Forbidden ownership

- raw source, unsaved buffers or query text in default logs
- hidden training or learning inputs
- treating green unit tests as product acceptance
- production crates depending on eval

## Allowed dependencies

`search-contracts`, `search-domain`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `record_metric(event) -> Result<(), EvalError>`
- `run_control_corpus(profile, baselines) -> EvaluationReport`
- `run_property_suite(target) -> PropertyReport`
- `audit_privileged_debug(trace_set) -> LeakageReport`
- `product_pulse_decision(report) -> AcceptanceDecision`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `EVALUATION_FIXTURE_INVALID`, `PRIVACY_LEAK_DETECTED`, `PRODUCT_ACCEPTANCE_NOT_PROVEN`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `default telemetry contains no source/query/path/corpus names`
- `required control-corpus cases are present`
- `A/B/C baselines use identical declared scope`
- `fault fixtures preserve raw receipts`
- `P15 cannot pass without security/resource/latency evidence`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W4 baseline / P08; acceptance W9 / P15**
- Soft `src/` target: **8,500 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.
