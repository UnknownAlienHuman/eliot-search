# Function contract — `search-model-provider`

**Status:** W10/P16 optional model contract; implementation, dependency and artifact selection remain
blocked until an exact accepted P15 receipt and candidate-specific ADR exist.

This package owns vendor-neutral model profile, encode/rerank request and output semantics. It does not
own the worker process, Qdrant transport, source/index stores, query planning, client authority,
qualification execution or G6 acceptance.

## Global rules

- one package instance serves one exact accepted `ModelProfileDescriptor`;
- no model, runtime, tokenizer, device backend or artifact is selected by this contract;
- provider output is a bounded scoring/nomination product, never source evidence or an answer;
- inputs are already authorized and bounded by server-owned planning;
- equal canonical input/profile/budget produces deterministic output where the profile claims
  determinism, otherwise an explicit bounded tolerance/equivalence receipt;
- no network, automatic artifact download/update, training, learning or persistent content cache;
- unsaved/source/query content never enters logs, debug, crash metadata or durable provider state;
- provider failure narrows optional coverage and never disables accepted P15 behavior.

## Profile operations

### `validate_profile_descriptor(candidate) -> Result<ValidatedModelProfile, ModelError>`

Requires exact candidate kind, provider/runtime/model/tokenizer/preprocessor artifacts, Windows package,
input templates, truncation, pooling/layout, dimensions, dtype, normalization, distance, quantization,
rerank semantics, finite resource limits, cache/content policy, license/source evidence and golden fixture
digest.

Rejects `latest`, floating revisions, implicit runtime/tokenizer defaults, zero/unbounded dimensions or
limits, unknown load-bearing fields, network requirements, generative-answer capability, provider-side
training/retention and profiles whose output cannot be validated independently.

Success validates the descriptor only; it does not qualify an artifact or authorize activation.

### `profile_digest(profile) -> ModelProfileId`

Uses domain-separated deterministic serialization covering every load-bearing field. Any model weights,
tokenizer/template, truncation, pooling, vector layout, dtype, normalization, quantization, calibration,
runtime, device or limit change produces a different profile ID.

### `validate_qualification_receipt(profile, receipt, accepted_p15) -> Result<QualifiedModelProfile, ModelError>`

Binds the descriptor to one accepted P15 report, dedicated ADR, exact artifact/runtime/Windows evidence,
document/query/rerank golden fixtures, content-policy audit and independent reviewer receipt. A receipt
for another profile, artifact, environment or P15 baseline is rejected.

### `describe_capability(profile, state) -> ModelProviderCapability`

Returns content-free profile/candidate kind, dimensions/vector names, accepted modalities, finite
limits, health/degradation and qualification identity. `AVAILABLE` is impossible before the daemon
activation receipt; package-local readiness alone is `QUALIFIED_NOT_ACTIVE`.

## Encoding inputs

### `validate_document_batch(request, profile, limits) -> Result<ValidatedDocumentBatch, ModelError>`

Requires one coherent installation/collection/profile/source-view/access/security fence, finite items and
bytes, exact source revision/unit identities, accepted modality/encoding and a stable operation/request
identity.

The package receives only the bounded content required by the profile. It cannot resolve paths, read
stores, widen scope or substitute current-path/Qdrant payload bytes.

### `validate_query_input(request, profile, limits) -> Result<ValidatedQueryInput, ModelError>`

Requires the query side of the same qualified profile, current request/binding fence and finite bytes or
tokens. Query text stays process-memory-only and is excluded from default diagnostics and receipts.

### `prepare_model_input(input, profile, budget, cancel) -> Result<ModelInputBatch, ModelError>`

Applies exactly the accepted normalization/template/tokenization/truncation policy and emits bounded
opaque worker input plus input/profile digests. It does not invent language, translate, summarize,
execute code or call another model.

Cancellation/budget exhaustion returns no successful partial input advertised as complete.

## Encode operations

### `encode_documents(batch, worker, context) -> Result<ModelVectorBatch, ModelError>`

Dispatches one exact profile and bounded batch through a model-worker port supplied by daemon
composition. The provider library has no process-launch or vendor-runtime dependency.

Success requires:

- one output per accepted input item or an explicit typed per-item failure set;
- exact output dimensions/vector layout/dtype/finite-value/normalization constraints;
- deterministic canonical item order;
- profile, request, input and worker-incarnation digests;
- bounded content-free resource/latency receipt;
- no source/query content in the public result.

A partial batch is never returned as an undifferentiated success and cannot enter publication without
caller-owned exact completeness handling.

### `encode_query(query, worker, context) -> Result<ModelQueryVector, ModelError>`

Uses the query side of the same profile. Output names/layout/dimensions and normalization match the
accepted collection/query fixture. Empty, non-finite, malformed or profile-mismatched output fails
closed; lexical fallback is a caller plan decision and is reported explicitly.

### `validate_vector_output(expected, observed, profile) -> Result<ValidatedModelVectors, ModelError>`

Checks shape, named-vector set, multivector token/segment order, dimensions, dtype, finite values,
normalization/tolerance, item identities and output digest. Vendor response structs and hidden metadata
cannot cross the package API.

### `vector_projection_receipt(vectors, profile) -> ModelProjectionReceipt`

Produces a content-minimized immutable receipt sufficient for projection planning and readback:
profile/input/output digests, named vectors, dimensions, item count, deterministic/tolerance class and
worker qualification identity. It contains no model input or source/query text.

## Rerank operations

### `validate_rerank_request(request, profile, limits) -> Result<ValidatedRerankRequest, ModelError>`

Requires candidate kind `RERANK_ONLY` or a profile with accepted rerank capability, one authorized query
fence and a finite candidate set whose members already passed the required server-owned eligibility and
source-backed validation stage declared by the plan.

Every candidate carries a stable opaque identity and bounded permitted text/features. Duplicate or
foreign candidates, raw Qdrant scores/IDs, inaccessible metadata and unvalidated gaps are rejected.

### `rerank(request, worker, context) -> Result<RerankOutput, ModelError>`

May reorder or drop only members of the input candidate set. It cannot add candidates, widen scope,
change handles/evidence roles, convert a gap into evidence, claim completeness, synthesize an answer or
emit a client disposition.

Success returns canonical candidate identity order, bounded finite scores or rank positions under the
accepted calibration, dropped-item reasons, profile/request/input/output digests and a content-free
resource receipt.

### `validate_rerank_output(input, observed, profile) -> Result<ValidatedRerankOutput, ModelError>`

Proves output set is a subset of input, identities are unique, score/rank values are finite and within
the accepted semantics, tie-break is deterministic and no hidden candidate/metadata was introduced.

### `apply_rerank_failure_policy(input_order, failure, profile) -> RerankFailureDecision`

Returns one explicit closed decision selected by the server plan:

```text
PRESERVE_PRE_RERANK_ORDER_WITH_GAP
DROP_OPTIONAL_LEG_WITH_GAP
FAIL_REQUEST
```

It never invokes a different model or silently returns reranked success.

## Planning and migration classification

### `classify_profile_capability(profile) -> OptionalModelPlanClass`

Returns exactly:

```text
RERANK_ONLY_NO_PERSISTENT_VECTOR
DENSE_NEW_COLLECTION_GENERATION
MULTIVECTOR_NEW_COLLECTION_GENERATION
```

Dense/multivector profiles require new projection profile set, point identities and collection
generation. Rerank-only requires no persistent-vector migration but still requires gate, worker,
configuration, benefit and removal receipts.

### `classify_profile_change(old, new) -> OptionalProfileChange`

- equal canonical profile: `NOOP`;
- any load-bearing change while inactive: `NEW_QUALIFICATION` plus generation requirement where
  applicable;
- active profile change: `DRAIN_REMOVE_AND_NEW_CANDIDATE`;
- invalid/unqualified change: `REJECT`.

In-place reinterpretation of stored vectors or score calibration is forbidden.

## Measurement and removal support

### `instrumentation_summary(operation_receipts) -> ModelInstrumentationSummary`

Aggregates bounded counts, dimensions, latency/resource classes, optional failure reasons and profile
identity for `search-eval`. It contains no content and cannot construct a benefit verdict.

### `validate_incremental_benefit_receipt(profile, receipt, accepted_p15) -> Result<BenefitReceipt, ModelError>`

Checks that an independent `search-eval` report compared this exact profile to the accepted P15 baseline
under pre-registered criteria, includes cost/risk and returns a material-benefit decision. The provider
package cannot create or self-accept this receipt.

### `prepare_removal(profile, active_state) -> Result<ModelRemovalPlan, ModelError>`

Returns the exact profile/vector/worker/cache/route obligations needed for daemon removal. It performs no
route switch or deletion. Persistent-vector profiles include exact collection-generation and manifest
references; rerank-only includes worker/cache/in-flight drain only.

### `validate_removal_receipt(plan, receipt, accepted_p15) -> Result<(), ModelError>`

Requires optional capability unavailable, worker stopped, provider-input/cache state cleared, optional
routes/vectors drained/reclaimed or explicitly deferred under pins, and accepted P15 regression digest.
It never claims physical secure erase without evidence.

## Cancellation, deadlines and retry

Preparation and validation are pure and retry-safe. Worker calls use explicit request identity, deadline,
cancellation and finite budgets. Cancellation before dispatch is clean. After dispatch, a late worker
reply is accepted only for the exact live request identity; otherwise it is discarded and content
buffers are released.

Encode/rerank operations have no durable Search side effect. Timeout/cancellation returns no successful
output and never advances publication or activation. A caller may retry the same canonical operation
under the accepted policy; a different input under the same identity is rejected.

Worker crash/unavailability returns a typed optional gap or request failure according to the plan. The
package never restarts the worker or changes profiles.

## Typed failures

- `OPTIONAL_DEPTH_NOT_ACCEPTED`
- `OPTIONAL_PROFILE_ADR_MISSING`
- `MODEL_PROVIDER_DISABLED`
- `MODEL_PROVIDER_UNAVAILABLE`
- `MODEL_ARTIFACT_NOT_QUALIFIED`
- `MODEL_PROFILE_INVALID`
- `MODEL_PROFILE_MISMATCH`
- `MODEL_INPUT_UNAUTHORIZED`
- `MODEL_INPUT_UNSUPPORTED`
- `MODEL_INPUT_LIMIT_EXCEEDED`
- `MODEL_OUTPUT_INVALID`
- `MODEL_OUTPUT_NONFINITE`
- `MODEL_VECTOR_SHAPE_MISMATCH`
- `MODEL_QUERY_DOCUMENT_INCOMPATIBLE`
- `RERANK_CANDIDATE_SET_INVALID`
- `RERANK_OUTPUT_NOT_SUBSET`
- `MODEL_BUDGET_EXHAUSTED`
- `MODEL_OPERATION_CANCELLED`
- `MODEL_WORKER_CRASHED`
- `MODEL_CONTENT_POLICY_VIOLATION`
- `MODEL_BENEFIT_NOT_PROVED`
- `MODEL_REMOVAL_INCOMPLETE`

## Required tests / qualification evidence

- no model/runtime/tokenizer/provider selected in scaffold;
- accepted P15 + candidate ADR + exact qualification required;
- canonical profile digest changes for every load-bearing dimension;
- document/query golden pair and rerank calibration fixture;
- deterministic/tolerance output and non-finite/shape rejection;
- multivector canonical token/segment ordering;
- rerank output is a strict input subset and cannot widen scope;
- provider output cannot become source evidence, exact identity or complete-negative proof;
- inaccessible/staged/retired/denied/purged/shadowed population noninterference;
- restrictive drift discards the influenced model leg;
- finite input/batch/concurrency/memory/deadline budgets;
- cancellation/timeout/crash yields no successful partial output;
- no network, auto-download/update, training/learning or persistent content cache;
- unsaved/source/query content absent from logs/debug/crash/receipts;
- rerank-only migration not-applicable proof;
- dense/multivector require new collection generation;
- measured material benefit receipt belongs to `search-eval`/reviewer;
- worker/provider removal restores accepted P15 behavior;
- public API contains no vendor runtime, Qdrant, redb, CAS or client-authority type.
