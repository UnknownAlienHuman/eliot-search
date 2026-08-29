# W4 bounded query product qualification contract

**Status:** `NOT_EXECUTED`  
**Architecture:** ELIOT Search 8.4, S14, S19–S23, S26, H12–H16, P08  
**Scope:** provider request lifecycle, grant/scope admission, exact snapshot/plan, bounded multi-leg
execution, access noninterference, source-backed validation, compact result projection, source handles,
continuations and baseline evaluation.

No unit-test collection, demo query or green compile is sufficient. Every mandatory probe in
[`probes.toml`](probes.toml) must execute against exact accepted package API digests, configuration
fingerprint and either DIRECT or the qualified W3 indexed profile named by the case.

## Owners

| Evidence | Owner |
|---|---|
| frame/session/replay/in-flight/cancel lifecycle | `search-provider-protocol` |
| grant/scope/eligibility/live deny/noninterference | `search-access` |
| recipe normalization, S14 snapshot and plan fingerprints | `search-query-planner` |
| lanes, budgets, pins, cancellation, fusion and partial coverage | `search-retrieval-executor` |
| exact revision readback and validation gaps | `search-candidate-validator` |
| opaque source-handle state and expansion authorization | `search-handles` |
| compact recipe results, result budgets and coverage | `search-result-projector` |
| opaque continuation state, pins and replan checkpoints | `search-continuation` |
| control corpus, leakage audit and raw evidence aggregation | `search-eval` |
| end-to-end composition and truthful degradation | `eliot-searchd` |

One package cannot accept its own evidence. The integration owner binds exact commits/API digests and
an independent reviewer receipt.

## Frozen inputs

Before execution, publish immutable identities for:

- repository commit, Rust toolchain and `Cargo.lock`;
- accepted `search-contracts`, `search-domain`, `search-ports` and every direct dependency API digest;
- effective configuration fingerprint and section digests;
- DIRECT and, where used, qualified W3 route/epoch/profile receipts;
- control-corpus manifest and every fixture digest;
- platform/process identity and resource envelope;
- probe registry digest and baseline descriptor digest.

Mutable branch heads or locally edited corpora are invalid inputs.

## Execution order

1. Validate frame, hello, binding and grant admission without opening stores.
2. Intersect requested scope with one immutable authoritative registry snapshot.
3. Compile canonical base eligibility and prove retrieval/IDF population equivalence.
4. Capture the exact S14 query snapshot and compile a deterministic bounded plan.
5. Admit work under finite queue/lane/resource ceilings and acquire only required pins.
6. Execute DIRECT and accepted indexed legs through vendor-neutral ports.
7. Trigger cancellation, deadline, saturation, provider failure and live revocation at every checkpoint.
8. Validate nominations by exact source-revision readback; invalid nominations become non-evidence gaps.
9. Project compact recipe results and opaque source handles under deterministic disclosure budgets.
10. Exercise ephemeral and durable continuation variants, restart/expiry and live-fence drift.
11. Audit telemetry, errors, receipts, handles and tokens for content/secret/path leakage.
12. Publish raw outputs plus independent review. Only then may P08/G2 product-slice evidence be accepted.

## Mandatory properties

### Admission and noninterference

- grant/binding/expiry/revocation and recipe/budget ceilings are checked before planning;
- client scope can only narrow server-authoritative memberships;
- access, currentness, purge and shadow predicates apply before candidates, IDF, counts, diversity and
  traces;
- inaccessible corpus changes do not alter authorized ordering, counts or trace;
- a newly denied population contaminates the whole influenced leg; post-filter-only cleanup is forbidden;
- ordinary reads write no durable redb lease/history/idempotency row.

### Snapshot and planning

- every named S14 axis is explicit and fingerprinted;
- direct-only and indexed route/epoch forms are distinct and valid only under their tagged contracts;
- strict-current recipes reject unresolved observation gaps;
- equal canonical inputs yield byte-identical snapshot/plan fingerprints and leg DAG;
- client vendor filters/collections/point IDs cannot enter the plan;
- all leg, candidate, source-read, result, CPU/memory and deadline ceilings are finite.

### Execution and validation

- queues/prefetch/retries are bounded and interactive work has priority;
- cancellation/disconnect releases every pin and request-local resource;
- raw scores never cross scoring populations; cross-leg fusion is profile-pinned and deterministic;
- saturation, timeout and provider failure remain explicit partial coverage;
- every evidence candidate is reopened from the exact source revision and verified for digest, length,
  anchor, unit, profile, residency and live security state;
- stale/unreadable/revoked/purged nominations carry no evidence excerpt or evidence-bearing handle.

### Results, handles and continuations

- only validated candidates enter evidence-bearing fields;
- `complete_scope` requires exact-plane proof over the authoritative denominator;
- result truncation is deterministic and records omissions;
- default candidate cards recommend a bounded 2–4 exact source handles where eligible;
- public handle/continuation tokens reveal no source, path, binding, plan, fence, cursor, score, point ID
  or residency fields;
- possession never grants access; every expansion reauthorizes live state;
- ephemeral state is memory-only and restart-invalid;
- durable source handles require retained immutable revisions; durable continuation checkpoints are
  explicit durable jobs, own no process pin and never target unsaved bytes;
- expired or drifted continuation fences return explicit refresh/expiry, never silent newer-corpus continuation.

## Stop conditions

Any of the following keeps P08 unavailable:

- post-filter-only security or access-influenced score/IDF/count leakage;
- missing S14 axis or nondeterministic plan fingerprint/order;
- unbounded queue, leg, prefetch, result, token or source-read path;
- raw-score fusion across populations;
- leaked pin/resource on cancel/disconnect;
- Qdrant/cached payload accepted as evidence without exact readback;
- validation gap containing excerpt/evidence handle;
- complete-scope claim without exact proof;
- self-contained or logged handle/continuation token;
- silent continuation against a newer fence;
- missing raw output, `UNAVAILABLE` mandatory probe or self-review.

## Evidence products

Each run records exact command/fixture, commit/API/config/profile identities, platform/resource envelope,
start/end time, `PASS | FAIL | UNAVAILABLE`, raw output digest and reviewer. Prose-only evidence is
rejected.

## Current disposition

```text
package implementations: ABSENT
control corpus: DESIGNED_NOT_EXECUTED
probe results: UNAVAILABLE
DIRECT query product: NOT_QUALIFIED
indexed query product: NOT_QUALIFIED
P08/G2 product slice: BLOCKED
```
