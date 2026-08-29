# W9 Product Pulse and Windows qualification contracts 1.0

**Status:** implementation projection only; P15/G5 remains blocked.  
**Architecture:** ELIOT Search 8.4, S29.1, S30, S35-S37, H14-H17/P15.  
**Owner:** `search-eval` owns evaluation meaning; the integration owner executes the accepted product;
an independent reviewer accepts or rejects the evidence.

## 1. Purpose and authority

P15 answers one bounded question:

```text
Does the qualified DIRECT/LEXICAL/CODE baseline beat or materially complement the declared baselines on
one immutable control corpus, without stale/access/content leakage, within the accepted Windows resource
budget, and with correct recovery under the mandatory fault matrix?
```

P15 does not change production ranking, contracts, source authority, access policy or lifecycle
semantics. Evaluation observations are not training data and cannot flow into production scoring,
lexical profiles, fixtures or query-specific tuning.

Only one closed Product Pulse outcome can satisfy G5:

```text
ACCEPTED
```

`REJECTED` is a complete negative decision. `INCOMPLETE`, `UNAVAILABLE` and any blocked state are
truthful non-acceptance states and cannot authorize optional depth.

## 2. Roles

### 2.1 `search-eval`

Owns:

- immutable corpus/run/baseline/metric/evidence schemas;
- deterministic validation and aggregation;
- leakage and telemetry audits;
- paired A/B/C comparison semantics;
- report and verdict construction;
- explicit visibility of every failed, skipped and unavailable observation.

It does not own production provider behavior, fault injection into another package, Windows process
control, hidden tuning, external-tool installation or gate self-acceptance.

### 2.2 Integration owner

Owns the dev/test driver that:

- freezes exact repository, dependency/API, configuration, artifact and Windows environment identities;
- invokes A, B and C only through their accepted user/provider surfaces;
- executes fault, stress, leakage and resource procedures;
- stores immutable raw outputs and digests;
- assembles evidence references without reinterpreting failed results.

The driver is not a production runtime dependency and exposes no new Search authority surface.

### 2.3 Independent reviewer

Reviews the criteria before candidate C results are visible, reproduces a bounded sample or full run,
checks immutable raw evidence and signs the final accept/reject receipt. The producer and reviewer cannot
be the same identity.

## 3. Frozen control corpus

The corpus manifest is immutable for one Product Pulse run and includes all Architecture S35 classes:

- actively edited local function;
- exact and renamed analogues across at least eight independent reference lineages;
- same-name false positive;
- decisive tests and separate documentation/caller/configuration evidence roles;
- mutually exclusive configuration variants;
- fork and mirror collapse;
- nested repository and submodule boundaries;
- stale, unindexed and inaccessible sources;
- saved and authenticated unsaved edits;
- watcher gap and resume reconciliation;
- every publication failpoint;
- access revocation during query;
- purge/restore non-resurrection;
- point-identity collision;
- multilingual documentation and non-ASCII paths.

The machine inventory is `qualification/product-pulse/corpus.toml`. Every case binds exact fixture and
oracle digests before execution. Empty/mutable fixture refs keep the corpus `UNMATERIALIZED`.

### 3.1 Oracle independence

Oracle labels are prepared and reviewed independently from the Search candidate outputs. Production
packages, baseline drivers and query generation never receive oracle answers. A run is invalid if an
oracle label, expected handle, decisive file list or acceptance threshold influenced indexing, ranking,
query construction or post-hoc case selection.

### 3.2 Scope parity

A, B and C receive the same declared source portfolio, source/workspace view, inclusion/exclusion policy,
disclosure ceiling, no-network policy, time boundary and question intent. A tool that cannot express the
scope records the exact limitation; the harness does not silently shrink the denominator or remove hard
cases.

## 4. Baselines

```text
A — pinned raw grep/read procedure
B — pinned current comparison/code-search tool
C — the exact ELIOT Search commit and qualified DIRECT/LEXICAL/CODE profile
```

Each `BaselineDescriptor` records:

```yaml
baseline_id: A | B | C
artifact_name_and_version: string
artifact_digest: digest
source_or_release_ref: immutable_ref
configuration_digest: digest
preparation_state: cold | warm
network_policy: disabled
scope_capability: exact | declared_limitation
operator_or_driver_digest: digest
```

`latest`, floating revisions, undocumented local patches and mixed versions are rejected. B remains
`UNSELECTED` until an integration ticket identifies the exact comparison tool and rationale.

### 4.1 Preparation cost

Index/build/preparation time, peak resources and disk growth are reported separately from query latency.
Warm query results never erase cold-start or preparation cost. A baseline may not reuse C's private
index, oracle data or inaccessible source state.

## 5. Windows qualification environment

One run binds an exact `WindowsEnvironmentIdentity`:

```yaml
os_edition_version_build: string
architecture: x86_64
machine_identity_class: opaque
cpu_model_and_logical_count: string
memory_bytes: u64
storage_model_bus_filesystem: string
system_volume_free_bytes: u64
gpu_model_driver: optional_string
power_plan_and_power_source: string
virtualization_state: string
antivirus_and_indexer_state: string
pagefile_state: string
locale_timezone_position_encoding: string
sleep_resume_capabilities: object
thermal_precondition: object
background_process_policy: object
clock_source: object
```

The run additionally binds repository commit, Cargo/dependency/API digests, Qdrant server/client
artifacts, lexical/parser/exact profiles, daemon/CLI artifacts, configuration fingerprint, corpus digest
and every accepted dependency qualification receipt.

Environment drift during a block invalidates comparability. A different Windows build, artifact,
configuration, corpus or provider profile is a different run, not another sample in the same block.

## 6. Experiment design

### 6.1 Blocked paired execution

Cases run in deterministic blocks. Within each block, A/B/C order is randomized from a captured seed.
Every baseline sees the same case snapshot. Warm-up observations are excluded but preserved in raw
output. Measured repetitions and sampling intervals are finite and pre-registered.

A cancelled or timed-out case is retained as a failed/incomplete observation. It is never silently
rerun until it passes; a retry is a new attempt linked to the original.

### 6.2 Cold and warm lanes

The report separates:

```text
cold installation/preparation
cold first request
warm steady-state request
background maintenance under idle policy
foreground request while background work is active
recovery after named fault
```

Cache-clearing and warm-state procedures are explicit. An unknown cache state invalidates the lane.

### 6.3 Comparable budgets

Each case records wall-clock deadline, source-read ceiling, output/token ceiling where applicable,
operator interaction allowance and network prohibition. Search retains its server-owned execution
budgets and truthful partial semantics. The harness cannot raise a Search grant or convert a partial
result into success to improve a score.

## 7. Metric registry

The canonical machine registry is `qualification/product-pulse/metrics.toml`.

Required quality and efficiency metrics include:

- correct grounded action rate;
- oracle definition/test/documentation/caller/configuration recall;
- false analogue rate;
- ambiguity and incomplete-coverage honesty;
- source reads and bytes;
- input/output tokens where the baseline uses a model;
- time to first correct grounded action;
- time to first useful progressive card;
- complete-negative correctness;
- stale/access/secret/content leakage counts.

Required performance/resource metrics include:

- request p50/p95 and captured tail distribution;
- process and system CPU;
- working set/private bytes/commit;
- disk bytes and write amplification;
- Qdrant/CAS/source reads;
- background CPU/disk duty cycle;
- queue depth, saturation and rejection counts;
- recovery time and recovery correctness.

Every metric declares unit, direction, denominator, aggregation and missing-value semantics. Metrics
from incompatible corpus/environment/profile identities cannot be combined.

## 8. Hard safety and correctness blockers

The following are zero-tolerance and bypass quality trade-offs:

- inaccessible source, membership name/count, source content or ranking influence leaks;
- stale/currentness overclaim;
- source/query/unsaved bytes, secrets, bearer tokens or unrestricted absolute paths in ordinary
  telemetry, raw protocol diagnostics, crash attachments or evidence metadata;
- false `NO_MATCH_IN_COMPLETE_SCOPE`;
- Qdrant payload accepted as source evidence without exact revision readback;
- hidden/failed probe omitted from aggregation;
- publication, pin/reclaim, purge/restore or access-revocation invariant failure;
- unbounded queue, frame assembly, source read, exact scan or result materialization;
- protocol replay accepted, more than one terminal result, or disconnect/cancel resource leak;
- test/oracle data entering production ranking or learning.

Any occurrence yields `BLOCKED_SAFETY` or `BLOCKED_CORRECTNESS`; G5 cannot pass even if aggregate quality
or latency is superior.

## 9. Candidate SLO checks

Architecture targets are evaluated as acceptance targets on the frozen Windows/corpus profile, not
universal SLA claims:

```text
warm exact/keyword navigation p95       <= 100 ms
warm single-scope lexical query p95     <= 200 ms
warm cross-repository comparison p95    <= 700 ms before source expansions
first useful progressive card           <= 300 ms when the local branch is ready
```

The exact timer boundaries and readiness preconditions are defined in the metric registry. A missing or
inapplicable required measurement is `UNAVAILABLE`, not a pass. Tail latency is reported with sample
count and uncertainty; p95 is not computed from fewer observations than the pre-registered policy.

## 10. Material product benefit

Architecture requires Search to beat **or materially complement** A/B without leakage and within
resource budgets. Numeric quality/efficiency thresholds are not invented by a package agent.

Before C results are available, the integration owner and reviewer freeze a `QualityAcceptancePolicy`
with:

- primary and secondary metrics;
- non-inferiority margins;
- minimum practical-effect thresholds;
- allowed resource regressions;
- case-family weights or an explicit unweighted rule;
- missing/failed observation handling;
- `DOMINATES` and `COMPLEMENTS` decision rules.

The policy digest is part of the run identity. Post-hoc threshold selection, dropping losing cases or
changing weights after seeing C invalidates the Product Pulse.

## 11. Fault and recovery matrix

The matrix consumes accepted package-local fault fixtures and adds end-to-end Windows execution for:

- daemon termination at each relevant durable/control boundary;
- every publication failpoint;
- Qdrant termination/restart and identity mismatch;
- redb/CAS reopen, corruption/quarantine and disk-full/low-space behavior;
- watcher overflow/reset, sleep/resume and root rebinding;
- access revoke during retrieval/readback/emission/handle expansion;
- purge then restore/reindex resurrection attempt;
- point-ID collision;
- continuation disconnect/restart expiry;
- background work under foreground pressure.

Each cell records expected state, observed state, recovery command, exact receipt and whether serving is
DIRECT-only, indexed, blocked or quarantined. Recovery latency without recovery correctness is not a
pass.

## 12. Protocol-flow-control stress

Stress remains local and authenticated. It covers:

- maximum and oversize frames, partial prefix/body and malformed UTF-8/JSON;
- replay/regressed sequences and duplicate request IDs;
- exactly 32 in-flight requests and bounded rejection of the 33rd;
- slow producer/consumer, cancellation storm and disconnect churn;
- progress sequencing and duplicate terminal prevention;
- relative deadlines and request-local pin/resource release;
- binding revoke/expiry during load;
- large evidence via handle expansion rather than oversized frames.

The test records peak memory, queue depth, leaked guards/pins, process stability and content-minimized
errors. A crash, unbounded growth, authority leak or false success is a hard failure.

## 13. Source-admission and leakage audit

The corpus contains synthetic credential/private-key/token canaries and inaccessible corpus canaries.
The audit scans:

```text
ordinary logs
privileged debug output
metrics and traces
Qdrant payload/readback diagnostics
redb/CAS technical records exposed to the harness
protocol frames and error metadata
process command lines and environment snapshots
crash dumps/attachments
backup and restore manifests
raw Product Pulse outputs and reports
```

Privileged debug is separately authenticated, binding-scoped and TTL-bounded. It still cannot expose
secrets or bearer tokens. Evaluation evidence stores content digests and case IDs; raw sensitive fixture
bytes remain in an access-controlled fixture store and are never copied into the report.

## 14. Evidence records

Every probe emits an immutable `EvidenceRef` containing:

```yaml
evidence_id: registry_id
probe_id: registry_id
producer: package_or_integration_owner
repository_commit: git_sha
dependency_api_digests: bounded_list<digest>
configuration_fingerprint: digest
corpus_and_case_digest: digest
windows_environment_digest: digest
command_or_fixture_digest: digest
result: PASS | FAIL | UNAVAILABLE
raw_output_ref: immutable_ref
raw_output_digest: digest
started_at_and_finished_at: timestamps
reviewer_receipt_ref: optional_until_review
bounded_non_content_notes: object
```

`PASS` requires raw output and independent review. Prose-only statements, screenshots without raw data,
mutable dashboard links or successful compilation are rejected.

## 15. Product Pulse report

`ProductPulseReport` contains:

- exact run, environment, corpus, criteria and baseline identities;
- every required probe and evidence reference;
- case-level outcomes and aggregate metrics;
- cold/warm/preparation/resource separation;
- failed, skipped, cancelled, timed-out and unavailable observations;
- fault/recovery and protocol-stress matrices;
- source-admission/content-minimization audits;
- architecture SLO checks;
- DOMINATES/COMPLEMENTS analysis under the pre-registered policy;
- limitations and capability exclusions;
- reproducibility digest.

The report never includes source/query/secret bytes or inaccessible names. Redaction cannot hide a
failure or change a denominator.

## 16. Verdict state machine

```text
DESIGNED_NOT_EXECUTED
  -> RUNNING
  -> EVIDENCE_COMPLETE
  -> UNDER_INDEPENDENT_REVIEW
  -> ACCEPTED | REJECTED

any state -> INCOMPLETE | BLOCKED_SAFETY | BLOCKED_CORRECTNESS |
             BLOCKED_REPRODUCIBILITY | BLOCKED_PERFORMANCE | BLOCKED_QUALITY
```

`decide_acceptance` is deterministic for one report and policy digest.

`ACCEPTED` requires:

1. every mandatory G5 probe is `PASS` with raw evidence;
2. every hard safety/correctness blocker is absent;
3. required candidate SLO checks pass;
4. the pre-registered policy returns `DOMINATES` or `COMPLEMENTS`;
5. environment/corpus/baseline identities are coherent;
6. an independent reviewer accepts the evidence and report digest.

Any missing required probe yields `INCOMPLETE`, not `REJECTED` or `ACCEPTED`. A complete run that fails
criteria yields `REJECTED` or the specific blocked state.

## 17. Optional-depth gate

No model/document/advanced-scale package may start from a green unit suite, partial report or informal
claim. The W10 ticket requires the exact accepted Product Pulse report and reviewer receipt digests.
Optional evaluation results cannot rewrite the accepted baseline; each optional profile must show
additional material benefit and an uninstall/rollback path.

## 18. Bounded agent decomposition

One package writer owns only `crates/search-eval/**`. Its ordinary W9 read set is:

1. package/root `AGENTS.md`;
2. package assignment and `FUNCTIONS.md`;
3. this cross-package contract;
4. `qualification/product-pulse/{baseline,corpus,metrics,probes,gate-map}.toml`;
5. `config/w9-product-pulse.toml`;
6. accepted direct dependency/API handoffs and immutable fixture refs.

The integration owner, not the package writer, edits shared qualification evidence after executing real
commands. An architecture contradiction stops work through the contract-change process.

## 19. Hard stop conditions

- mutable/unpinned baseline, corpus, criteria or environment;
- fewer than eight independent reference lineages in the accepted corpus;
- hidden case removal or post-hoc policy/weight change;
- oracle/training leakage;
- scope/view mismatch across A/B/C without explicit invalidation;
- mixed cold/warm or incompatible environment samples;
- any safety/correctness blocker;
- missing raw output or independent reviewer;
- optional profile enabled during baseline acceptance;
- optional depth authorized without exact `ACCEPTED` receipt.

## 20. Current disposition

```text
control corpus: UNMATERIALIZED
baseline A: UNSELECTED
baseline B: UNSELECTED
candidate C runtime: UNAVAILABLE
Windows environment: UNSELECTED
quality acceptance policy: UNSELECTED
G5 probes: NOT EXECUTED
Product Pulse: NOT ACCEPTED
optional depth: BLOCKED
```
