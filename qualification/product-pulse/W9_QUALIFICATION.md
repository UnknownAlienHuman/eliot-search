# W9 Product Pulse and Windows qualification

**Status:** `DESIGNED_NOT_EXECUTED`  
**Stage / gate:** P15 / G5  
**Architecture:** ELIOT Search 8.4 S29.1, S30, S35-S37 and P15  
**Evidence owner:** `search-eval`; execution owner: integration owner; acceptance owner: independent
reviewer.

## 1. Qualification subject

One qualification run binds exactly:

- repository commit and accepted W0-W8 API/handoff digests;
- DIRECT/LEXICAL/CODE artifacts and configuration fingerprint;
- exact Qdrant server/client, lexical, Rust parser and exact-predicate profiles;
- one immutable control corpus and oracle revision;
- exact A/B baseline artifacts/drivers/configurations;
- one exact Windows x64 environment identity;
- one pre-registered quality/resource acceptance policy;
- one run seed, warm-up/sample policy and immutable raw-output store.

Changing any load-bearing identity creates a new run. Results from different runs are not pooled unless
the metric registry explicitly defines a reviewed cross-run comparison.

## 2. Prerequisites

Execution is blocked until the integration ticket provides accepted receipts for:

1. G0 contracts;
2. G1 direct source/revision spine;
3. G2 lexical/Qdrant/publication/query baseline;
4. G3 current workspace, Rust structure, comparison and exact proof;
5. W7 security/lifecycle hardening, including purge/restore and durable handles;
6. G4 authenticated generic client edge;
7. exact Windows-compatible artifacts and dependency/API digests.

A package scaffold, compilation, unit test or unreviewed branch is not a prerequisite receipt.

## 3. Freeze order

The integration owner and independent reviewer perform this order before running C:

1. materialize and review every required `corpus.toml` case and oracle digest;
2. prove at least eight independent reference lineages plus explicit fork/mirror relations;
3. select exact baseline A and B artifacts/drivers and pin their digests;
4. freeze candidate C commit/artifacts/profile/configuration;
5. freeze the exact Windows environment profile;
6. freeze `metrics.toml` and a complete quality acceptance policy;
7. freeze run seed, warm-up/measured counts, cache lanes and resource sampling;
8. publish the frozen run manifest digest;
9. only then expose/run candidate C observations.

Any criteria, weight, case or baseline change after candidate observation invalidates the run.

## 4. Execution sequence

1. Verify all artifact/environment/corpus/policy digests.
2. Execute cold installation/preparation lanes and retain preparation evidence.
3. Execute paired randomized A/B/C case blocks with identical case snapshots.
4. Execute warm steady-state and background-pressure lanes.
5. Execute every mandatory fault/recovery cell.
6. Execute provider-protocol stress.
7. Execute source-admission, content-minimization and privileged-debug leakage scans.
8. Validate raw outputs, case metrics and environment continuity.
9. Aggregate the complete report without dropping failures/unavailable observations.
10. Independently reproduce the required sample or full run.
11. Construct and review the closed Product Pulse verdict.

Retries are linked attempts. The original failed/cancelled/timed-out observation remains visible.

## 5. A/B/C fairness

A, B and C receive the same declared source portfolio, source/workspace view, inclusion policy,
disclosure ceiling, question intent, network prohibition and time boundary. Tool limitations are recorded
as limitations, not repaired by narrowing the corpus.

Preparation/index build cost is reported separately. Warm query latency cannot erase cold/preparation
cost. No baseline may receive oracle answers, C's private index, inaccessible source state or query-specific
hand tuning unavailable to the others.

## 6. Hard blockers

G5 cannot pass after any confirmed:

- stale/currentness overclaim;
- inaccessible content, membership metadata or ranking influence;
- source/query/unsaved/secret/token/path leakage on an audited surface;
- false complete-negative proof;
- source evidence emitted from Qdrant payload without exact revision readback;
- publication, pin/reclaim, access, purge/restore or owner invariant failure;
- protocol replay/authority violation, duplicate terminal event or leaked request resources;
- unbounded queue/frame/source-read/exact-scan/result growth;
- oracle/evaluation feedback into production behavior;
- hidden failed, skipped or unavailable observation;
- mutable/unpinned evidence or producer self-acceptance.

Hard blockers are not weighted metrics and cannot be traded for quality or speed.

## 7. Performance and resource evidence

The Architecture S30.2 values are candidate acceptance targets on the frozen profile:

```text
warm exact/keyword navigation p95       <= 100 ms
warm single-scope lexical query p95     <= 200 ms
warm cross-repository comparison p95    <= 700 ms before source expansions
first useful progressive card           <= 300 ms when local branch is ready
```

Evidence includes sample count, timer boundaries, readiness/cache state, p50/p95 distribution, CPU,
working set/private bytes/commit, disk and source/CAS/Qdrant I/O, preparation cost, queue/saturation and
background duty cycle. Unknown cache, thermal, power or process identity makes the lane unavailable.

## 8. Fault and recovery evidence

The end-to-end matrix covers every publication failpoint and required process/storage/currentness/
security/lifecycle fault. Each cell records expected/observed state, exact receipts, recovery action,
serving mode and recovery latency. Recovery speed cannot compensate for wrong visibility, access,
currentness, retention or quarantine state.

## 9. Protocol stress evidence

Stress covers max/oversize/partial/malformed frames, sequence and request replay, the 32 in-flight
ceiling, progress/terminal ordering, cancellation storm, connection churn, deadline expiry, binding
revocation and large handle expansion. It records peak memory/queues and leaked requests, guards and
pins. Any crash, unbounded growth, authority leak or false success fails the probe.

## 10. Leakage evidence

Synthetic canaries, never real user credentials, cover source bodies, query text, unsaved bytes,
credentials/private keys, bearer tokens, absolute paths and inaccessible scope names/counts. Audited
surfaces include ordinary/privileged logs, metrics/traces, protocol errors, Qdrant diagnostics, exposed
redb/CAS metadata, process command/environment, crash artifacts, backup/restore metadata and Product
Pulse outputs.

Privileged debug remains authenticated, binding-scoped and TTL-bounded and still cannot expose secrets
or bearer tokens.

## 11. Evidence record

Every PASS record contains stable evidence/probe IDs, producer, exact commit/API/config/corpus/environment
identities, command/fixture digest, immutable raw-output ref/digest, timestamps and independent reviewer
receipt. Prose-only evidence, screenshots without raw data, mutable dashboards and successful compilation
are rejected.

## 12. Verdict

`ACCEPTED` requires all sixty probes PASS, zero hard blockers, complete required SLO evidence,
DOMINATES or COMPLEMENTS under the pre-registered policy, coherent identities and independent review.

Missing evidence yields `INCOMPLETE`. Complete evidence that misses criteria yields `REJECTED` or a
specific blocked state. Only the exact accepted report/reviewer receipt may satisfy G5 or appear in a W10
optional-depth assignment ticket.

## 13. Stop conditions

Stop without acceptance when:

- any required prerequisite is absent;
- corpus/baseline/environment/policy remains `UNSELECTED` or `UNMATERIALIZED`;
- a required probe is FAIL or UNAVAILABLE;
- environment or artifact drift occurs;
- scope/view parity cannot be established;
- fewer than eight independent lineages are proven;
- criteria or case selection changed after C observation;
- raw output or independent review is absent;
- any hard blocker is present.

## 14. Current disposition

```text
prerequisites: NOT ACCEPTED FOR W9 EXECUTION
control corpus: UNMATERIALIZED
baseline A/B: UNSELECTED
candidate C runtime: UNAVAILABLE
Windows environment: UNSELECTED
quality policy: UNSELECTED
mandatory probes: 60 UNAVAILABLE
Product Pulse: NOT ACCEPTED
optional depth: BLOCKED
```
