# Function contract — `search-eval`

**Status:** W4 baseline qualification contract; Product Pulse acceptance remains W9/P15.

This package owns deterministic evaluation schemas, content-minimized telemetry audits and verdict
construction. It does not modify production ranking or convert unit tests into product acceptance.

## Corpus and run operations

### `load_control_corpus(manifest, fixture_store) -> Result<ControlCorpus, EvalError>`

Validates immutable fixture digests, expected source/access/view states, recipe requests, oracle class,
resource ceilings and disclosure policy. Unknown or mutable fixture inputs fail closed.

### `validate_probe_registry(registry) -> Result<ValidatedProbeRegistry, EvalError>`

Rejects duplicate IDs, missing owners, prose-only evidence, unsupported result values and mandatory
probes without commands/fixtures/raw-output requirements.

### `run_case(provider, case, environment, budget, cancellation) -> Result<CaseResult, EvalError>`

Captures exact repository/API/configuration/artifact/environment identities, executes one bounded case
and preserves PASS/FAIL/UNAVAILABLE plus raw-output digest. It never records source/query/secret/path
content in ordinary telemetry.

### `run_probe_suite(provider, probes, environment, budget) -> QualificationRun`

Runs mandatory probes independently; one failure or unavailable result keeps the corresponding gate
unaccepted. Ordering never permits an earlier pass to hide a later failure.

### `compare_baselines(raw_read, direct, lexical) -> BaselineComparison`

Compares exact raw read/grep baseline, DIRECT and accepted lexical product on quality, coverage,
latency/resource and leakage dimensions. Metrics bind one control corpus revision and cannot be mixed
across incompatible runs.

## Audit and verdict operations

### `audit_content_minimization(events, policy) -> LeakageAudit`

Detects source/query/unsaved bytes, secrets, opaque tokens, absolute paths and inaccessible display
metadata. A leak is a failing result, not a warning.

### `aggregate_product_pulse(runs, criteria) -> ProductPulseReport`

Requires complete immutable raw evidence, failed/skipped visibility and reproducible environment IDs.
Missing required evidence yields an incomplete report.

### `decide_acceptance(report) -> AcceptanceVerdict`

Only W9/P15 may emit the Product Pulse accept/reject decision. Optional semantic/document work remains
blocked unless the report satisfies every accepted criterion and receives independent review.

## Observability configuration

Implements `config/sections/observability.md`. Ordinary logging remains content-minimized. Privileged
debug requires explicit authenticated scope, finite TTL and security-barrier receipt; it never permits
secret/token logging.

## Required fixtures

Manifest/digest validation; duplicate/missing probe rejection; raw-read/DIRECT/lexical A/B/C fixture;
PASS/FAIL/UNAVAILABLE preservation; access/stale/ambiguity/overlay/continuation cases; content leakage
negative corpus; telemetry redaction; incomplete evidence rejects verdict; production ranking cannot
import evaluation oracle data.
