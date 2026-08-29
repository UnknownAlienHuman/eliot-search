# Function contract — `eliot-search-model-worker`

**Status:** W10/P16 isolated optional worker contract; no model/runtime/artifact is selected and the
binary remains absent/stopped until an accepted P15 receipt, ADR and exact qualification authorize one
candidate profile.

The worker hosts one qualified local model provider behind a private daemon-owned channel. Provider
meaning belongs to `search-model-provider`; activation, process supervision, route/configuration and
client capability publication belong to `eliot-searchd`.

## Startup

### `validate_startup(config, gate_receipts, artifact_evidence) -> Result<ValidatedModelWorkerConfig, WorkerError>`

Requires exact accepted P15 report, candidate ADR, model profile and qualification receipts, compiled
feature identity, worker binary/runtime/model/tokenizer digests, Windows package/license evidence,
private channel identity and finite CPU/GPU/memory/queue/deadline/cache policies.

Rejects `latest`, floating revisions, plaintext secrets, network requirements, automatic download/update,
training/learning, persistent input cache, multiple unqualified profiles and any direct store/index/client
endpoint.

### `verify_inherited_containment(config, process_context) -> Result<ContainmentReceipt, WorkerError>`

Verifies installation/process incarnation, daemon parent/owner, private endpoint, filesystem ACL,
Job Object or accepted lifecycle boundary, working/temp directories and effective resource policy before
loading model artifacts. A responding channel or PID alone is insufficient.

### `load_qualified_provider(config, budget, cancel) -> Result<ModelWorkerReady, WorkerError>`

Opens only the exact local artifacts in the qualified package, verifies digests after open, constructs
one profile instance and executes bounded startup/golden self-checks. It performs no network discovery,
package resolution or model download.

Cancellation/timeout after partial load performs bounded provider/context cleanup. Readiness is
`WORKER_READY_NOT_ACTIVE`; daemon gate state still controls serving.

### `open_private_session(ready, daemon_hello) -> Result<ModelWorkerSession, WorkerError>`

Mutually binds daemon/worker installation incarnation, process identities, profile digest, protocol
major/minor, nonce/sequence and finite frame/in-flight limits. The channel accepts no external client.

## Request lifecycle

### `admit_request(session, envelope, limits) -> Result<ModelRequestGuard, WorkerError>`

Validates monotonic sequence, unique request identity, exact profile/incarnation, operation kind,
deadline, cancellation capability, input byte/item/token ceilings and per-session/global queue limits
before allocation/provider invocation.

Only typed document encode, query encode and rerank bodies accepted from the daemon are valid. Paths,
store handles, Qdrant/redb/CAS clients, client grants, shell commands and arbitrary provider options are
not representable.

### `serve_encode(guard, request, provider, resource_guard) -> Result<ModelWorkerResponse, WorkerError>`

Invokes the accepted provider under finite memory/commit/CPU/GPU/time limits and returns bounded opaque
vectors plus profile/input/output/resource digests. It validates shape/finite values before framing but
does not replace the library's authoritative semantic validation.

Content buffers remain request-local and are released on terminal state. stdout/stderr, metrics and
health contain no model input, source/query text, tokens or bearer values.

### `serve_rerank(guard, request, provider, resource_guard) -> Result<ModelWorkerResponse, WorkerError>`

Processes only the finite candidate identities and bounded features/text supplied by the daemon. Output
can contain only input candidate identities and accepted score/rank values. The worker cannot add a
candidate or synthesize an answer.

### `cancel_request(session, request_id) -> CancelOutcome`

Idempotently signals provider cancellation, stops further output emission and releases request-local
content/resource reservations. Unknown/already-terminal requests return bounded non-sensitive outcomes.

If the provider ignores cancellation past the accepted grace period, the daemon may terminate the worker
through its process boundary. The worker never claims clean cancellation while a terminal response may
still be emitted; terminal state is serialized exactly once.

### `emit_terminal(guard, response_or_error) -> Result<WorkerTerminalReceipt, WorkerError>`

Emits exactly one bounded terminal event for the request. Late provider callbacks after cancellation,
deadline or terminal state are discarded and their content buffers released.

## Health, pressure and lifecycle

### `health_snapshot(session, resources) -> ModelWorkerHealth`

Returns profile/artifact/incarnation identities, state, finite queue/in-flight counts, memory/CPU/GPU
resource classes, restart/quarantine reasons and content-free error codes. It never includes input,
model prompt/template expansion or absolute artifact paths.

### `apply_resource_pressure(policy, observation) -> WorkerPressureDecision`

May reduce admission/concurrency, pause optional work, reject with `RESOURCE_EXHAUSTED` or request daemon
restart/drain. It cannot raise accepted limits, evict baseline daemon resources or silently run work after
deadline.

### `begin_drain(session, reason) -> ModelWorkerDrainGuard`

Stops new admission, allows only bounded accepted completion/cancellation of current requests and reports
remaining work. Drain is finite; exceeding the deadline requests daemon-enforced termination.

### `shutdown_and_remove(guard, cleanup_policy) -> Result<ModelWorkerShutdownReceipt, WorkerError>`

Cancels/drains work, releases provider/GPU/runtime contexts, closes the private channel, removes bounded
provider-input/temp/cache state allowed by policy and verifies no child/background process remains.

The receipt distinguishes graceful, forced-verified and incomplete/quarantined cleanup and contains no
content. It does not switch Search routes/configuration or claim baseline restoration; daemon integration
owns that proof.

## Crash and retry semantics

The worker owns no durable Search state. A process crash invalidates all sessions and request outputs not
already accepted by the daemon. The daemon reports optional degradation, restarts only under the exact
accepted profile and bounded policy, or quarantines.

A request is retryable only with the same canonical operation/profile/input identity after the caller has
classified the prior request as having no accepted terminal output. The worker does not persist an
idempotency database or recover content requests after restart.

Startup ambiguity is resolved by exact daemon/process/artifact/session identity, never by attaching to an
arbitrary pipe or provider process.

## Typed failures

- `OPTIONAL_DEPTH_NOT_ACCEPTED`
- `MODEL_WORKER_PROFILE_DISABLED`
- `MODEL_WORKER_ARTIFACT_MISMATCH`
- `MODEL_WORKER_CONTAINMENT_FAILED`
- `MODEL_WORKER_PRIVATE_CHANNEL_FAILED`
- `MODEL_WORKER_PROTOCOL_MISMATCH`
- `MODEL_WORKER_REPLAY_REJECTED`
- `MODEL_WORKER_REQUEST_INVALID`
- `MODEL_WORKER_RESOURCE_EXHAUSTED`
- `MODEL_WORKER_DEADLINE_EXCEEDED`
- `MODEL_WORKER_CANCELLED`
- `MODEL_WORKER_PROVIDER_FAILED`
- `MODEL_WORKER_OUTPUT_INVALID`
- `MODEL_WORKER_CRASHED`
- `MODEL_WORKER_QUARANTINED`
- `MODEL_WORKER_SHUTDOWN_INCOMPLETE`
- `MODEL_CONTENT_POLICY_VIOLATION`

## Required tests / qualification evidence

- binary/feature absent from accepted baseline and worker stopped by default;
- P15/ADR/profile/artifact/runtime/Windows receipt chain required;
- exact artifact re-open digest and provider golden startup fixture;
- private daemon-only authentication, replay and frame/in-flight limits;
- no redb/CAS/Qdrant/source/handle/secret-store/client dependency or request field;
- finite input/token/batch/queue/concurrency/memory/commit/CPU/GPU/deadline bounds;
- encode/rerank output shape/finite/subset guards;
- cancellation before, during and ignored-by-provider grace behavior;
- exactly one terminal response and late-callback discard;
- content absent from command line/environment/stdout/stderr/health/log/crash receipt;
- no network, auto-download/update, training/learning or persistent input cache;
- worker crash cannot crash daemon and yields explicit optional degradation;
- bounded restart reaches quarantine;
- shutdown clears provider input/temp/cache state and verifies process exit;
- removal fixture proves daemon can restore exact accepted P15 behavior without this binary.
