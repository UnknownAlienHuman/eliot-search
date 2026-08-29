# Function contract — `search-eval`

**Status:** W4 baseline schemas plus W9/P15 Product Pulse contract; no evaluation runtime or acceptance
evidence exists yet.

`search-eval` owns deterministic evaluation meaning, evidence validation, leakage audits and verdict
construction. Cross-package execution is performed by an integration-owned dev/test driver through
accepted public/provider interfaces. Production packages never depend on this crate.

## Global rules

- all inputs are bounded, immutable and identified by canonical digests;
- oracle labels, acceptance thresholds and case selection are hidden from production and baseline tools;
- A/B/C comparisons use one frozen scope/view/corpus and exact artifact/configuration identities;
- cold, warm, preparation and recovery observations remain separate;
- failed, skipped, cancelled, timed-out and unavailable observations remain visible;
- ordinary telemetry and reports contain no source/query/unsaved/secret/token/absolute-path content;
- package or integration producers cannot independently accept their own evidence;
- equal canonical evidence and policy inputs produce byte-identical reports and verdicts.

## Corpus and policy operations

### `validate_control_corpus(manifest, fixture_index) -> Result<ValidatedControlCorpus, EvalError>`

Requires every registered S35 case class, exact fixture/oracle digests, at least eight independent
reference lineages, explicit fork/mirror/nested/submodule relations, source/view/access/currentness states,
finite case budgets and disclosure policy. Mutable, missing, duplicate or oracle-contaminated fixtures
fail closed.

Success validates schema/completeness only; it does not claim cases are correctly implemented by C.

### `validate_metric_registry(registry) -> Result<ValidatedMetricRegistry, EvalError>`

Rejects duplicate metrics, absent unit/direction/denominator, undefined missing-value behavior, unsafe
aggregation, incompatible percentile rules and metrics that permit leakage to be averaged away.
Zero-tolerance safety metrics are structurally separate from quality/performance trade-offs.

### `validate_acceptance_policy(policy, metric_registry, gate) -> Result<ValidatedAcceptancePolicy, EvalError>`

Requires pre-registration before candidate C results, explicit primary/secondary metrics,
non-inferiority and practical-effect rules, resource ceilings, case-family weighting, missing/failure
handling, DOMINATES/COMPLEMENTS definitions and independent reviewer approval. Post-hoc or self-approved
policy is rejected.

### `freeze_run_manifest(input, corpus, metrics, policy) -> Result<FrozenRunManifest, EvalError>`

Binds repository commit, dependency/API digests, configurations, all artifacts/profiles, exact Windows
environment, baseline descriptors, corpus, run seed, repetitions, warm-up/cache rules and raw-output
store identity. Any load-bearing `UNSELECTED` field prevents execution authorization.

### `validate_baseline_descriptor(descriptor, run) -> Result<ValidatedBaseline, EvalError>`

Requires exact source/release/version/digest, configuration, operator/driver digest, no-network policy,
declared scope capability and cold/warm preparation state. `latest`, floating revisions, hidden patches
and reuse of candidate/oracle-private state are invalid.

## Execution-plan and evidence ingestion

### `plan_case_block(run, cases, block_index) -> Result<CaseExecutionBlock, EvalError>`

Creates a deterministic randomized A/B/C order from the frozen seed, with identical case snapshots and
finite deadlines/resource/output ceilings. Warm-ups and measured attempts are labeled separately.
Planning performs no provider invocation.

### `run_case(driver, baseline, case, attempt, context) -> Result<CaseExecutionEvidence, EvalError>`

This operation is available only to dev/test integration. `driver` is a package-owned test seam that
invokes the accepted external surface and captures raw outputs; it is not a production adapter or
store/database port.

Cancellation before invocation is clean. Cancellation/deadline after invocation preserves the partial
raw trace and returns a cancelled/timed-out observation; it never silently retries until success. Equal
attempt identity with different input is rejected.

### `validate_case_evidence(evidence, run, case, baseline) -> Result<ValidatedCaseEvidence, EvalError>`

Checks all identities, timing boundaries, scope/view parity, output digest, terminal state, resource
sample continuity, disclosure classification and raw-output reference. A successful tool exit cannot be
relabeled PASS when the oracle, coverage or safety expectations fail.

### `ingest_external_probe(probe, evidence_ref, run) -> Result<ValidatedProbeEvidence, EvalError>`

Consumes package/integration fault, recovery, protocol, source-admission and leakage evidence. `PASS`
requires immutable raw output and an independent reviewer receipt; `UNAVAILABLE` and `FAIL` remain
first-class outcomes.

### `reproduce_evidence(evidence, environment, driver) -> Result<ReproductionReceipt, EvalError>`

Re-executes the exact command/fixture on a matching environment or validates an explicitly permitted
reproduction profile. Drift or non-reproducible output is reported, not hidden by aggregation.

## Metric operations

### `score_case(evidence, oracle, registry) -> Result<CaseMetricSet, EvalError>`

Computes only pre-registered metrics. Every metric retains numerator, denominator, missing/failed counts
and provenance. Source handles are expanded only through accepted authorization and raw fixture bytes
never enter the metric record.

### `aggregate_block(cases, registry, run) -> Result<BlockMetrics, EvalError>`

Aggregates only coherent corpus/environment/profile identities. It preserves baseline, cold/warm lane,
case family and attempt status. Percentiles require the pre-registered minimum sample count; otherwise
they are `UNAVAILABLE`.

### `compare_abc(a, b, c, policy) -> Result<BaselineComparison, EvalError>`

Performs paired case-level comparison under the frozen policy. It cannot drop losing cases, change
weights, widen C's grant or treat unsupported baseline scope as success. Output explicitly classifies
`DOMINATES`, `COMPLEMENTS`, `NON_INFERIOR_WITHOUT_MATERIAL_GAIN`, `REGRESSES`, or `INCOMPLETE`.

### `evaluate_candidate_slos(metrics, slo_registry) -> Result<SloReport, EvalError>`

Evaluates the four Architecture S30.2 candidate targets using exact timer boundaries and readiness
preconditions. The report is scoped to the qualified Windows/corpus profile and is never advertised as a
universal SLA.

### `compute_resource_report(samples, run) -> Result<ResourceReport, EvalError>`

Reports CPU, memory, disk, source/CAS/Qdrant I/O, queue/saturation, preparation cost and background duty
cycle. Sampling gaps, counter reset, thermal/power/environment drift and mixed process identity make the
relevant result incomplete.

## Security, leakage and recovery audits

### `audit_content_minimization(events, canaries, policy) -> Result<LeakageAudit, EvalError>`

Scans ordinary/privileged logs, metrics, traces, protocol errors, command lines, environment snapshots,
crash artifacts, backup/restore metadata and evaluation outputs for source/query/unsaved bytes, secrets,
bearer tokens, unrestricted paths and inaccessible metadata. Any confirmed hard canary is a failure,
not a warning or averaged metric.

### `audit_source_admission(observations, expected) -> Result<AdmissionAudit, EvalError>`

Verifies deny-by-default credential/private-key/system-location behavior, sensitivity/disclosure ceilings
and absence from payload/index/telemetry surfaces. It does not read or publish real user secrets.

### `validate_fault_matrix(matrix, required_cells) -> Result<FaultRecoveryReport, EvalError>`

Requires every registered publication, process, storage, observation, security, purge/restore,
continuation and resource-pressure cell. Each cell contains expected state, observed state, exact receipt,
recovery action and latency. Correct latency cannot compensate for an incorrect state transition.

### `validate_protocol_stress(report, policy) -> Result<ProtocolStressReport, EvalError>`

Checks framing/replay/in-flight/cancellation/disconnect/deadline/binding-revoke stress, peak memory,
queue depth, terminal-event uniqueness and leaked guard/pin counts. Crash, unbounded growth, authority
leak or false success is a hard failure.

## Report and verdict operations

### `aggregate_product_pulse(run, cases, probes, metrics, audits) -> Result<ProductPulseReport, EvalError>`

Requires the complete mandatory registry. The report includes exact identities, all raw evidence refs,
case-level and aggregate metrics, cold/warm/preparation separation, failure/unavailable visibility,
fault/protocol/security matrices, SLO checks, DOMINATES/COMPLEMENTS analysis, limitations and a canonical
reproducibility digest.

A report with missing evidence is returned as `INCOMPLETE`; it is not rejected as a product nor accepted.

### `detect_hard_blockers(report) -> HardBlockerSet`

Returns closed blocker classes for stale/access/secret/content leakage, false complete-negative claims,
publication/pin/purge/restore/revocation failures, protocol authority/flow-control failures, unbounded
resources, oracle/training contamination and reproducibility drift. Hard blockers cannot be waived by a
weighted quality score.

### `decide_acceptance(report, policy, review) -> Result<AcceptanceVerdict, EvalError>`

Returns one closed state:

```text
ACCEPTED
REJECTED
INCOMPLETE
BLOCKED_SAFETY
BLOCKED_CORRECTNESS
BLOCKED_REPRODUCIBILITY
BLOCKED_PERFORMANCE
BLOCKED_QUALITY
```

`ACCEPTED` requires every mandatory G5 probe PASS with immutable raw evidence, no hard blocker, required
SLO success, DOMINATES or COMPLEMENTS under the pre-registered policy, coherent run identities and an
independent reviewer receipt. The producer cannot supply the accepting review.

### `issue_product_pulse_receipt(verdict, report, review) -> Result<ProductPulseReceipt, EvalError>`

Emits a content-minimized immutable receipt binding report/policy/environment/corpus/baseline digests,
verdict, reviewer and G5 evidence refs. Only an `ACCEPTED` receipt may be consumed by W10 optional-depth
tickets. Reissuing equal input is deterministic; changing input requires a new receipt and never mutates
an accepted historical receipt.

## Observability configuration

Implements `config/sections/observability.md` for production telemetry and consumes the separate
evaluation-only `config/w9-product-pulse.toml`. Production logging floors cannot be weakened by the
harness. Privileged debug requires explicit authenticated scope, finite TTL and security-barrier receipt;
it still cannot expose secrets or bearer tokens.

## Cancellation, retry and crash semantics

- schema/validation/aggregation operations are pure and retry-safe;
- case execution retries create linked attempts and preserve the original failure;
- lost evidence publication is resolved from immutable artifact identity/digest, not by overwriting;
- cancellation never removes a denominator item or failed observation;
- an interrupted run resumes from the frozen manifest and completed immutable attempts;
- environment, corpus, criteria or artifact drift requires a new run identity;
- report/verdict publication is append-only and content-addressed.

## Typed failures

- `EVALUATION_FIXTURE_INVALID`
- `EVALUATION_CORPUS_INCOMPLETE`
- `EVALUATION_BASELINE_UNQUALIFIED`
- `EVALUATION_ENVIRONMENT_DRIFT`
- `EVALUATION_SCOPE_MISMATCH`
- `EVALUATION_ORACLE_CONTAMINATION`
- `EVALUATION_POLICY_NOT_PREREGISTERED`
- `EVALUATION_ATTEMPT_CONFLICT`
- `EVALUATION_RAW_EVIDENCE_MISSING`
- `EVALUATION_REPRODUCTION_FAILED`
- `EVALUATION_METRIC_UNAVAILABLE`
- `EVALUATION_INCOMPLETE`
- `LEAKAGE_DETECTED`
- `FAULT_MATRIX_INCOMPLETE`
- `PROTOCOL_STRESS_FAILED`
- `PRODUCT_NOT_ACCEPTED`
- `SELF_ACCEPTANCE_FORBIDDEN`
- `OPTIONAL_DEPTH_NOT_AUTHORIZED`

## Required tests / qualification evidence

- complete registered S35 corpus and eight-lineage minimum;
- mutable/missing/oracle-contaminated fixture rejection;
- exact artifact/configuration/environment/run identity;
- deterministic seeded A/B/C order and attempt identity;
- identical declared scope/view and explicit baseline limitation;
- cold/warm/preparation separation;
- failed/cancelled/timed-out/unavailable observations cannot disappear;
- percentile minimum sample and incompatible-run aggregation rejection;
- post-hoc criteria/weight/case change rejection;
- no oracle or evaluation feedback into production ranking;
- zero-tolerance stale/access/secret/content canaries;
- complete-negative false-claim blocker;
- all publication/process/storage/currentness/security/purge/restore fault cells;
- protocol replay/oversize/32-in-flight/cancel/disconnect stress and resource bounds;
- candidate SLO timer-boundary fixtures;
- DOMINATES/COMPLEMENTS policy golden;
- incomplete report cannot produce acceptance;
- hard blocker cannot be waived by aggregate score;
- producer cannot self-review;
- accepted receipt is immutable and is the sole W10 prerequisite;
- public/debug report contains no source/query/secret/token/path content;
- production crates have no dependency on `search-eval`.
