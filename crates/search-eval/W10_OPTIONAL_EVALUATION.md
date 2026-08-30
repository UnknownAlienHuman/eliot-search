# W10 reentry — optional candidate evaluation in `search-eval`

**Status:** package-stage delta only; G6 remains blocked.  
**Base handoff:** exact accepted W9/P15 `search-eval` commit/API plus final independently accepted Product
Pulse receipt.  
**Write owner:** one `search-eval` package agent for one selected candidate profile.

This delta extends the accepted baseline evaluation machinery for one exact optional model, document or
scale candidate. It does not select artifacts, change baseline P15, implement provider behavior or
self-accept G6.

## Reentry and candidate identity

One ticket binds exactly one:

```text
model_rerank:<profile digest>
model_dense:<profile digest>
model_multivector:<profile digest>
document:<profile digest>
scale:<profile digest>
```

The agent reads only the accepted W9 public handoff, package assignment/base `FUNCTIONS.md`, this file,
W10 stage/configuration/qualification packets and exact candidate/fixture receipts. It does not reread
W9 implementation internals or unrelated candidate classes.

Changed provider, runtime, model/tokenizer/templates/quantization, document engine/limits/maps or scale
topology creates a new campaign and package ticket. The cumulative crate line budget is not reset.

## Operations

### `validate_optional_campaign(spec, accepted_p15, candidate_receipts, policy) -> Result<ValidatedOptionalCampaign, EvalError>`

Requires exact accepted P15 report/reviewer receipt, dedicated candidate ADR, exact artifact/profile/
Windows/license qualification, candidate class/profile digest, pre-registered incremental-benefit policy,
removal/fallback plan and migration/rollback receipt or reviewed rerank-only not-applicable receipt.

Rejects mutable/floating identities, mixed candidate classes, self-issued evidence, thresholds selected
after candidate results, baseline configuration drift and optional candidate already influencing the
accepted P15 corpus/results.

### `freeze_candidate_comparison(candidate, baseline, extension_cases, metric_registry, policy, artifact_port, operation) -> Result<FrozenOptionalComparison, EvalError>`

Binds:

- exact accepted P15 baseline build/profile/configuration/corpus/metrics;
- exact candidate build/profile/artifacts/feature/configuration;
- pre-registered case extension and candidate use classes;
- identical authorized scope/source/workspace view and disclosure;
- paired baseline-vs-candidate trial schedule, budgets, cache/machine profile and randomization;
- benefit/non-inferiority/resource/risk/removal thresholds;
- candidate-specific security/content/fault probes.

The baseline lane has the candidate disabled/absent. The candidate lane differs only by the exact staged
optional profile and declared migration state. Same operation is idempotent; conflicting freeze input
rejects.

### `validate_candidate_fixture_extension(extension, candidate_class, accepted_p15) -> Result<ValidatedCandidateFixtureExtension, EvalError>`

Requires positive/negative/ambiguity/security/removal cases not already covered by baseline. Cases remain
source-backed and independently labeled; candidate output cannot define the oracle.

Minimum candidate-specific families:

```text
model: true/false analogues, access/currentness noninterference, truncation, calibration, provider failure
document: fidelity/coordinates/loss/assurance, malformed/active/remote/bomb input, provider failure
scale: bottleneck reproduction, scoring/IDF/access equivalence, migration/failpoint/rollback, pin drain
all: feature disabled baseline, cancellation/crash, content leakage, complete removal and P15 regression
```

### `build_optional_trial_schedule(campaign, baseline_lane, candidate_lane, cases, repetitions, seed) -> Result<OptionalTrialSchedule, EvalError>`

Creates deterministic paired, randomized and finite baseline/candidate trials with separate cold/warm/
preparation/recovery lanes. It preserves all failed, timed-out, cancelled and unavailable attempts under
the pre-registered denominator rules.

For persistent candidates, migration/build/catch-up/storage cost is reported separately and cannot be
hidden by warm query latency.

### `ingest_candidate_operation_receipt(receipt, campaign, candidate) -> Result<ValidatedCandidateOperationEvidence, EvalError>`

Consumes content-minimized immutable receipts from model/document workers, daemon, publication, Qdrant,
pins/reclaimer and removal owners. It verifies exact candidate/profile/artifact/config/environment/
operation identity and rejects a receipt from another candidate or baseline.

The evaluator never opens provider caches, model inputs, source stores, Qdrant or migration state directly.

### `score_incremental_quality(baseline_trials, candidate_trials, oracle, policy) -> Result<OptionalQualityReport, EvalError>`

Computes only pre-registered paired metrics, preserving case family and candidate class.

Model reports include accepted recall/precision/false-analogue or ranking gain without treating model
scores as evidence. Document reports include materialization/evidence fidelity, coordinate/loss-map and
assurance correctness. Scale reports quality/scoring/IDF equivalence or an explicitly accepted new
product/scoring profile.

### `score_incremental_cost(baseline_resources, candidate_resources, policy) -> Result<OptionalCostReport, EvalError>`

Reports latency to first useful result, steady-state tails, CPU/GPU, RAM/VRAM, disk, preparation/migration,
source reads, provider input/token counts where applicable, background duty, queue/saturation and
operational failure/restart cost.

Missing GPU/provider/resource counters required by the policy remain `UNAVAILABLE`; they are not assumed
zero. Baseline absence of optional worker/GPU work is verified separately.

### `audit_optional_noninterference(observations, canaries, policy) -> Result<OptionalNoninterferenceAudit, EvalError>`

Zero-tolerance checks include:

- inaccessible/stale/denied/purged/shadowed population influencing nomination, IDF, fusion, rerank,
  counts, traces or calibration;
- model/provider result presented as exact source evidence or complete-negative proof;
- rerank adding a candidate, widening scope or changing evidence identity;
- source/query/unsaved content in persistent cache, telemetry, command lines, environment or crash data;
- document script/macro/OLE/hook/filter/shell/child process/remote resource/network behavior;
- document coordinate/loss/assurance overclaim or current-path/Qdrant-payload substitution;
- scale scoring/access/currentness mismatch, staged visibility or alias-as-commit;
- hidden provider fallback or automatic artifact update/download;
- baseline capability becoming dependent on the optional provider.

Any confirmed hard finding blocks G6 regardless of quality gain.

### `validate_optional_fault_matrix(matrix, candidate_class, required_cells) -> Result<OptionalFaultReport, EvalError>`

Requires exact worker/provider crash, cancellation/deadline/resource exhaustion, daemon restart,
activation/control/snapshot boundary and complete removal cases.

Persistent candidates additionally require candidate-generation/build/catch-up/final-barrier/route-switch/
pin-drain/reclaim/rollback failpoints. Recovery correctness and visible capability/route state are scored
separately from recovery latency.

### `validate_removal_and_p15_regression(removal, restored_baseline, accepted_p15, policy) -> Result<RemovalRegressionReport, EvalError>`

Requires:

- optional capability unavailable/draining before removal;
- new requests routed to the accepted P15 handler/profile/route/configuration;
- bounded in-flight completion/cancellation;
- worker/process exit and allowed input/temp/cache cleanup;
- route-pin drain and exact optional manifest reclaim or explicit deferred state;
- no secure-erasure overclaim;
- exact P15 regression fixture and capability/configuration digest match.

A candidate cannot pass G6 when uninstall/removal leaves baseline behavior, disk state or authority
uncertain.

### `compare_optional_candidate(quality, cost, noninterference, faults, removal, policy) -> Result<OptionalCandidateComparison, EvalError>`

Applies the frozen decision rules and returns one closed result:

```text
MATERIAL_BENEFIT
NON_INFERIOR_WITHOUT_MATERIAL_GAIN
REGRESSION
INCOMPLETE
BLOCKED_SAFETY
BLOCKED_COST
BLOCKED_REMOVAL
BLOCKED_REPRODUCIBILITY
```

A candidate that is merely different, expensive without accepted gain or operationally unsafe remains
disabled.

### `build_g6_evidence_candidate(campaign, comparison, evidence_index) -> Result<G6EvidenceCandidate, EvalError>`

Constructs all five central candidate-specific evidence records:

```text
dedicated_optional_profile_adr
exact_provider_artifact_qualification
measured_material_benefit
removal_or_uninstall_fallback
migration_and_rollback_when_applicable
```

For rerank-only, the final record may pass only with an independently reviewed
`NOT_APPLICABLE_NO_PERSISTENT_SCHEMA` receipt; it cannot be omitted.

The output is an immutable candidate bundle and never final `G6_ACCEPTED`.

### `verify_g6_independent_review(bundle, review) -> Result<OptionalIndependentReviewReceipt, EvalError>`

Verifies reviewer identity/separation, recalculated metrics/denominators, hard-blocker audit, exact
candidate profile, evidence completeness and final bundle digest. `search-eval` validates the receipt
shape but cannot choose the reviewer or activate the candidate.

## Configuration interaction

`config/w10-optional-depth.toml` supplies only exact qualified refs and bounded next-run resource limits.
Configuration alone cannot start an evaluation, select a provider or mark evidence PASS.

A candidate/profile/policy/artifact/corpus/environment change creates a new campaign. Active trials are
never live-reconfigured. Optional feature state is observed and verified, not controlled by this crate.

## Cancellation, retry and recovery

Pure validation/scoring is deterministic and retry-safe. Trial/evidence publication uses the accepted W9
campaign/operation machinery. Failed/cancelled/timed-out observations remain visible. Unknown artifact or
external mutation outcome is resolved by the owning package's exact receipt/readback; evaluation never
reruns until pass or invents rollback.

Recovery resumes from immutable campaign/evidence identities only when candidate/baseline/environment/
policy remain exact; drift invalidates the campaign.

## Typed failures

- `OPTIONAL_EVAL_P15_NOT_ACCEPTED`
- `OPTIONAL_EVAL_CANDIDATE_NOT_SELECTED`
- `OPTIONAL_EVAL_CANDIDATE_IDENTITY_MISMATCH`
- `OPTIONAL_EVAL_ADR_OR_QUALIFICATION_MISSING`
- `OPTIONAL_EVAL_POLICY_NOT_PREREGISTERED`
- `OPTIONAL_EVAL_BASELINE_DRIFT`
- `OPTIONAL_EVAL_FIXTURE_EXTENSION_INVALID`
- `OPTIONAL_EVAL_TRIAL_INCOMPLETE`
- `OPTIONAL_EVAL_OPERATION_RECEIPT_MISMATCH`
- `OPTIONAL_EVAL_QUALITY_REGRESSION`
- `OPTIONAL_EVAL_RESOURCE_COST_UNACCEPTABLE`
- `OPTIONAL_EVAL_NONINTERFERENCE_FAILED`
- `OPTIONAL_EVAL_FAULT_MATRIX_INCOMPLETE`
- `OPTIONAL_EVAL_REMOVAL_INCOMPLETE`
- `OPTIONAL_EVAL_P15_REGRESSION_FAILED`
- `OPTIONAL_EVAL_MATERIAL_BENEFIT_NOT_PROVED`
- `OPTIONAL_EVAL_G6_EVIDENCE_INCOMPLETE`
- `OPTIONAL_EVAL_SELF_ACCEPTANCE_FORBIDDEN`
- `OPTIONAL_EVAL_CANCELLED`

## Required tests / qualification evidence

- exact accepted W9/P15 package/API/report/reviewer prerequisites;
- one candidate class/profile per campaign and identity-drift rejection;
- thresholds/case extension frozen before candidate results;
- paired deterministic baseline/candidate schedule and failure-denominator preservation;
- model score/rerank cannot become source evidence, completeness or widened candidate scope;
- document no-execute/remote/bomb/coordinate/loss/assurance hard blockers;
- scale access/currentness/scoring/IDF and migration equivalence;
- preparation/migration/resource costs remain separate from warm query latency;
- missing required counters/evidence remain unavailable;
- all hard safety/content/authority findings dominate quality;
- provider failure leaves accepted baseline serviceable and explicit;
- complete removal, worker/cache/temp/pin/manifest state and P15 regression;
- rerank-only migration evidence present as reviewed not-applicable;
- exact five G6 evidence records and independent-review separation;
- no provider/store/Qdrant/daemon concrete dependency or content-bearing receipt;
- package-only diff and cumulative line-budget/split-review guard.
