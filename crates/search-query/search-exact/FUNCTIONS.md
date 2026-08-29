# Function contract — `search-exact`

**Status:** W6/P12 logical contract; no exact predicate engine or executor implementation exists yet.

The exact plane compiles one immutable authorized denominator and executes a pinned exact predicate over
every denominator item. It may prove only the stated predicate. Qdrant candidates, lexical top-k and
semantic similarity never define or narrow the denominator.

## State and authority

The package owns:

- exact predicate/profile validation and canonical identity;
- denominator freeze/manifest and exact-plan compilation;
- bounded item execution/checkpointing;
- match/failure accounting and completeness classification;
- source-backed exact report and complete-negative semantics.

It owns no source registry, revision/CAS store, access authority, overlay bytes, Qdrant client, regex
vendor adapter state or client verification/admission decision. All source inventory/readback and live
access operations occur through accepted vendor-neutral ports.

## Predicate profiles

Closed baseline predicate kinds:

```text
literal
regex
qualified_symbol
structural_pattern
record_field
```

Closed input domains:

```text
raw_bytes
decoded_text
structural_ir
```

A predicate profile binds exact engine/provider name, version/source checksum/license receipt, syntax
and flags, normalization/encoding policy, complexity class, size/depth/time/match limits, cancellation
behavior, serialized-form schema and golden fixture digest.

Regex requires a pinned non-backtracking engine/profile. A user pattern is never delegated to an
unbounded backtracking runtime. Structural predicates require an accepted structural-IR/profile digest;
they do not silently fall back to lexical matching.

## `validate_predicate_profile`

```text
validate_predicate_profile(candidate, qualification_receipt)
    -> Result<QualifiedExactPredicateProfile, ExactError>
```

Rejects unspecified/floating engine identity, unsupported/unknown load-bearing fields, unbounded
complexity/resources, ambiguous encoding/normalization, backreferences/lookaround/features outside the
qualified safe profile, hidden locale behavior and incomplete golden evidence.

Success validates the exact predicate semantics only; it does not authorize a source scope or prove a
negative result.

## `compile_predicate`

```text
compile_predicate(request_predicate, profile, bounds)
    -> Result<CompiledExactPredicate, ExactError>
```

Produces canonical versioned serialized form, predicate digest, input domain, engine/profile identity,
worst-case complexity class and effective bounds.

Equal canonical predicate/profile inputs produce equal compiled bytes/digest. Invalid syntax or a
pattern exceeding bounds fails before denominator acquisition.

## `freeze_denominator`

```text
freeze_denominator(requested_scope, authorized_scope, source_view, inventory_port,
                   overlay_snapshot, completeness, budget, cancel)
    -> Result<FrozenDenominator, ExactError>
```

**Preconditions**

- grant permits exact scan and requested recipe/predicate domain;
- requested scope is explicitly intersected with authoritative membership/source state;
- one coherent source/workspace view, owner generations, access/live-deny/purge/shadow and observation
  fence is captured;
- inventory revision is authoritative for the requested denominator kind;
- authenticated unsaved buffers are included only when explicitly requested/permitted and exact snapshot
  IDs are available.

**Postconditions**

- denominator contains exact ordered `SourceRevisionId`/item references plus inventory revision,
  membership/source-view/security/currentness/overlay digests and item count;
- each item records required read domain/encoding/profile and retained/stable revision requirement;
- duplicate source revision/items are deterministically eliminated only under exact identity rules;
- omitted/denied/unknown scope is explicit and prevents complete-scope classification where required;
- digest covers the exact item list and every load-bearing fence.

Qdrant point IDs/candidates, search ranking and client-supplied file lists cannot enter the denominator.
Cancellation or budget exhaustion returns no frozen denominator advertised as complete.

## `compile_exact_scan`

```text
compile_exact_scan(request, predicate, denominator, completeness, budget)
    -> Result<ExactScanPlan, ExactError>
```

Binds plan/request IDs, compiled predicate/profile, exact frozen denominator/digest, inclusion policy,
unsaved snapshot IDs, completeness requirements, access/security/currentness fences, plan budget,
expiry and `PlanFingerprint`.

`require_current_observation` rejects a relevant unresolved gap. Relaxed/stale observed mode may support
an explicitly incomplete search report but never `NoMatchInCompleteScope`.

Compilation is pure after captured ports/inputs. Same canonical inputs produce the same plan fingerprint;
new plan identity/expiry may remain explicit non-semantic fields as defined by contracts.

## `validate_plan_before_execution`

```text
validate_plan_before_execution(plan, current_inventory, current_security, current_overlays)
    -> PlanExecutionPermit
```

Checks plan expiry, denominator/inventory/source-view/owner/access/purge/shadow/observation/overlay and
predicate-profile identity before any read. Restrictive security change fails immediately. Non-security
scope drift either uses exact retained planned revisions or marks affected items/scope incomplete; it
never silently replaces them with current revisions.

## `execute_item`

```text
execute_item(plan, item, revision_store_port, structural_port, access_port, budget, cancel)
    -> ExactItemResult
```

Execution steps:

1. recheck current access/live deny/purge and planned item identity;
2. reopen the exact planned retained/stable revision or exact authenticated unsaved snapshot;
3. verify revision/content digest, byte length and required coordinate/representation/profile identity;
4. execute only the compiled predicate in its declared input domain and bounds;
5. produce source-backed `ExactMatch` values with exact anchors/digests or one explicit item failure;
6. recheck restrictive security before exposing match data.

Reading whatever currently occupies a path is forbidden. Qdrant payload text is never scanned/cited.
Raw-byte and decoded-text semantics remain distinct; decoding/transcoding loss is explicit.

Cancellation/deadline/budget during an item produces `cancelled`, `timeout` or `predicate_error` item
failure and prevents complete negative proof.

## `execute_exact_scan`

```text
execute_exact_scan(plan, ports, checkpoint, budget, cancel)
    -> Result<ExactExecutionReport, ExactError>
```

Processes every denominator item in canonical order or records why it did not. Work is bounded and may
checkpoint after exact item/batch receipts. The report contains:

- plan/predicate/denominator/source-view/security/currentness/profile digests;
- denominator count/digest;
- completed/matched/failed/unread/cancelled/timed-out/drifted counts;
- bounded exact source-backed matches;
- one failure record per uncompleted denominator item;
- scope/currentness/security drift observations;
- start/end/cancellation/deadline/resource summaries;
- explicit `candidate_scope | complete_scope | unknown` denominator classification;
- execution receipt digest.

A partial/cancelled report is returned as typed data when possible; it is never relabeled complete or
silently discarded into a generic no-match result.

## `checkpoint_execution`

```text
checkpoint_execution(plan, completed_items, results, operation, control_port)
    -> Result<ExactCheckpoint, ExactError>
```

Durable checkpoints are allowed only for governed verification jobs and contain plan/denominator/
predicate digests, completed exact item IDs and bounded non-content result refs. Unsaved bytes, source
bodies and process-local pins are never persisted.

Checkpoint commit uncertainty is resolved by operation readback. Ordinary interactive exact scans may
remain process-local and restart from the frozen retained denominator when still valid.

## `resume_execution`

```text
resume_execution(plan, checkpoint, current_fences, ports, budget, cancel)
    -> Result<ExactExecutionReport, ExactError>
```

Verifies exact plan/checkpoint/denominator/profile identities and rechecks access/purge/owner/currentness.
It resumes only unfinished items and validates prior receipts. Changed scope cannot add new items to the
frozen denominator; changed/unavailable planned revisions become failures.

An unsaved snapshot that expired or was invalidated blocks complete resume. No process-local pin is
assumed to survive restart.

## `classify_completeness`

```text
classify_completeness(plan, report, current_fences)
    -> ExactCoverage
```

`NO_MATCH_IN_COMPLETE_SCOPE` requires all of the following:

- authoritative frozen denominator for the explicitly stated scope;
- every required denominator item completed/readable under the exact planned revision/snapshot;
- zero matches;
- zero item failures, omissions, timeout, cancellation or provider gaps;
- exact predicate/profile semantics fully executed;
- no disqualifying source/scope/inventory/owner/access/purge/shadow/overlay/currentness drift;
- all required completeness flags true;
- report digest/receipt verification succeeds.

Any missing condition yields `INCOMPLETE_NO_MATCH`, `MATCHES_FOUND` or `EXECUTION_INVALID`, never a
complete negative claim. The proof concerns only the compiled predicate/input domain, not an arbitrary
semantic analogue.

## `verify_execution_report`

```text
verify_execution_report(plan, report, inventory_receipt, readback_receipts, current_security)
    -> Result<ExactVerificationReceipt, ExactError>
```

Recomputes plan/report/denominator/predicate digests, item accounting and completeness. Every denominator
item appears exactly once as completed result or failure; counts must match. Every emitted match has a
valid exact source handle/anchor/readback receipt and current emission authorization.

Verification cannot upgrade an incomplete report. Client adapters may map the receipt to their own
verification workflow but cannot alter its predicate/scope/coverage semantics.

## `revalidate_complete_negative`

```text
revalidate_complete_negative(receipt, current_fences) -> ExactProofRevalidation
```

Restrictive access/purge/owner changes invalidate emission/expansion immediately. Inventory/source-view/
overlay/observation/profile drift marks the proof stale for current-scope claims. A historical frozen
proof may remain valid only for its exact retained denominator and stated temporal/source-view scope.

## Cancellation, deadlines, idempotency and crash semantics

- predicate compilation/denominator canonicalization/report verification are pure and deterministic;
- cancellation before a frozen plan yields no plan;
- cancellation after plan creation yields explicit incomplete item/report accounting;
- a possibly committed checkpoint is resolved by operation identity/readback;
- item execution is retry-safe only for the same plan/item/exact revision/profile/security requirements;
- a timeout never authorizes substitution of a newer revision or narrower denominator;
- daemon crash loses unsaved buffers/process-local pins; resume revalidates exact retained state;
- no result path turns `CANCELLED`, `TIMEOUT`, `UNREADABLE`, `REVISION_UNAVAILABLE`, `SCOPE_CHANGED` or
  `OBSERVATION_GAP` into complete negative proof.

## Typed failures and reasons

- `EXACT_REQUEST_INVALID`
- `EXACT_SCAN_NOT_AUTHORIZED`
- `EXACT_PREDICATE_UNSUPPORTED`
- `EXACT_PREDICATE_INVALID`
- `EXACT_ENGINE_NOT_QUALIFIED`
- `EXACT_PREDICATE_LIMIT_EXCEEDED`
- `EXACT_SCOPE_EMPTY`
- `EXACT_DENOMINATOR_INCOMPLETE`
- `EXACT_DENOMINATOR_DRIFT`
- `SCOPE_CHANGED_OR_REVISION_UNAVAILABLE`
- `SOURCE_REVISION_UNAVAILABLE`
- `SOURCE_UNREADABLE`
- `EXACT_ENCODING_UNSUPPORTED`
- `EXACT_STRUCTURAL_PROFILE_MISMATCH`
- `EXACT_OBSERVATION_GAP`
- `EXACT_UNSAVED_SNAPSHOT_EXPIRED`
- `EXACT_ACCESS_REVOKED`
- `EXACT_PURGED`
- `EXACT_BUDGET_EXHAUSTED`
- `EXACT_TIMEOUT`
- `EXACT_CANCELLED`
- `EXACT_CHECKPOINT_OUTCOME_UNKNOWN`
- `EXACT_REPORT_INVALID`
- `INCOMPLETE_COVERAGE`
- `SEMANTIC_ABSENCE_CLAIM_FORBIDDEN`

## Required tests / qualification evidence

- literal raw-byte and decoded-text golden semantics remain distinct;
- exact qualified-symbol and structural-profile fixtures bind accepted IR/profile IDs;
- exact regex engine/version/source/license and safe non-backtracking feature corpus;
- catastrophic/backtracking-style patterns rejected or remain within proven bounds;
- denominator comes from authoritative inventory, never Qdrant/top-k/client file list;
- duplicate/overlapping memberships/items handled under exact identity without widening/narrowing;
- every denominator item appears exactly once in result/failure accounting;
- complete literal negative requires every item and zero failures;
- unreadable, revision unavailable, unsupported encoding, drift, timeout, cancellation and gap each
  independently block complete negative;
- live source change uses retained planned revision or returns scope/revision unavailable;
- restrictive access/purge between items and before emission blocks unauthorized match/report data;
- unsaved authenticated snapshot inclusion/expiry/restart matrix;
- checkpoint timeout-after-commit readback/idempotency and resume exactness;
- cancellation produces explicit incomplete report, not no-match success;
- historical frozen proof versus current-workspace stale revalidation;
- report/receipt tamper, count, denominator and digest mismatch rejection;
- semantic analogue absence overclaim rejected;
- fake inventory/revision/access/structural/control ports prove no concrete adapter/Qdrant/redb dependency;
- public API contains no regex/vendor/storage type or source dump.
