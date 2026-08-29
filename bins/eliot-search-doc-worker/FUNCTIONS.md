# Function contract — `eliot-search-doc-worker`

**Status:** W10/P17 isolated optional document-worker contract; no provider/runtime/artifact is selected
and the binary remains absent/stopped until accepted P15, a dedicated provider ADR and exact Windows
qualification authorize one profile.

The worker hosts one exact document materializer in a private no-execute process. Canonical source and
materialization meaning remain in Search contracts/materializer ports; activation, source acquisition,
revision storage, projection publication and client capability state remain outside this binary.

## Startup and profile admission

### `validate_provider_profile(config, gate_receipts, evidence) -> Result<ValidatedDocumentWorkerConfig, DocWorkerError>`

Requires accepted P15, candidate ADR, exact provider binary/library/runtime/Windows package/license,
input MIME/encoding set, archive/container policy, page/object/image/table/figure/decompression limits,
script/macro/hook/remote-resource/network policy, output schema, coordinate/loss-map identity,
assurance/warning taxonomy, temp/content policy, finite resource limits and golden/fuzz fixture digest.

Rejects implicit provider defaults, floating versions, plaintext secrets, unsupported input ambiguity,
network/credential requirements, executable behavior, unbounded archive/page/object/image/output policy
and profiles whose coordinates/loss cannot be independently validated.

### `verify_inherited_sandbox(config, process_context) -> Result<DocumentSandboxReceipt, DocWorkerError>`

Verifies daemon/process/installation incarnation, private endpoint, filesystem ACL, lifecycle boundary,
private bounded temp root, effective no-network/no-child-process controls and resource policy before
loading provider code.

The worker cannot open Search data roots, stores or Qdrant. A provider requiring broader privileges is
not admitted.

### `load_qualified_provider(config, budget, cancel) -> Result<DocumentWorkerReady, DocWorkerError>`

Reopens exact local provider/runtime artifacts, verifies digests and runs bounded no-execute/golden/fuzz
startup probes. It performs no package resolution, download, plugin discovery or remote-resource fetch.

Readiness remains `WORKER_READY_NOT_ACTIVE`; daemon gate/profile state controls serving.

### `open_private_session(ready, daemon_hello) -> Result<DocumentWorkerSession, DocWorkerError>`

Binds daemon/worker identities, installation/process incarnation, profile digest, protocol version,
nonce/sequence and bounded frame/in-flight limits. External clients cannot connect.

## Request inspection

### `admit_materialization_request(session, request, limits) -> Result<DocumentRequestGuard, DocWorkerError>`

Requires one exact retained source revision digest/length, declared input kind/encoding, authorized
materialization profile, finite bytes and resource/deadline/cancellation context. Input bytes are sent by
the daemon; paths, URLs, store handles and Qdrant/redb/CAS clients are not accepted.

Duplicate/conflicting request identity, unsupported/ambiguous MIME, oversize input and profile mismatch
fail before provider invocation.

### `inspect_container_and_input(guard, bytes, profile, budget, cancel) -> Result<SafeDocumentInput, DocWorkerError>`

Performs bounded magic/type/container inspection and rejects:

- absolute, parent-traversal, device, alternate-stream or duplicate-normalized member paths;
- symlink, hardlink or reparse escape;
- excessive nesting, members, compressed/uncompressed bytes or decompression ratio;
- excessive pages, objects, images, dimensions, tables or figures;
- embedded executables, scripts, JavaScript, VBA/macros, OLE actions or launch metadata;
- external links, remote fonts/images/schemas/styles or credential prompts;
- malformed encodings/structures outside the accepted fail/degrade policy.

Inspection itself executes no archive member, hook, filter or external application.

## Materialization

### `materialize(guard, safe_input, provider, resource_guard) -> Result<DocumentWorkerProduct, DocWorkerError>`

Invokes only the exact accepted provider under finite CPU/memory/commit/disk/temp/output/deadline bounds.
The worker blocks network and child-process execution throughout the call.

Success returns bounded typed outputs permitted by the profile:

```text
normalized text
structure hierarchy
qualified tables/figures metadata
native anchors
coordinate maps
loss maps
assurance ceiling
warnings and omitted/degraded regions
```

Every output binds exact source revision/input digest and provider/profile/runtime identity. It contains
no arbitrary provider object or executable payload.

### `validate_materialization_output(expected, observed, profile) -> Result<ValidatedDocumentProduct, DocWorkerError>`

Checks source digest/length, output byte/item limits, UTF/encoding contracts, structure references,
anchor containment, page/bbox coordinate space, coordinate-map consistency, loss-map completeness,
assurance/warning taxonomy and canonical output digests.

It never fabricates raw-byte/page exactness when mapping is lossy. Missing/unreadable regions remain
explicit; a degraded product cannot be relabeled high fidelity.

### `emit_terminal(guard, product_or_error) -> Result<DocumentTerminalReceipt, DocWorkerError>`

Emits exactly one bounded terminal response. Provider output is accepted only after validation. Late
callbacks/results after cancellation/deadline/terminal state are discarded and temp/content buffers are
released.

## Cancellation, deadlines and cleanup

### `cancel_request(session, request_id) -> CancelOutcome`

Idempotently requests provider cancellation and prevents further successful emission. If provider code
does not stop within the accepted grace period, the daemon terminates the worker process boundary.

### `cleanup_request_workspace(guard, policy) -> Result<DocumentCleanupReceipt, DocWorkerError>`

Closes handles, removes bounded private temp files and verifies no provider child/background process or
remote session remains. Cleanup is required after success, failure, cancellation, timeout and provider
crash. Receipt contains digests/counts only, not source content or raw paths.

A timeout after the provider may have written temp output is not a materialization success. Only validated
terminal output is publishable; unverified temp state is deleted/quarantined.

## Health and lifecycle

### `health_snapshot(session, resources) -> DocumentWorkerHealth`

Returns profile/artifact/incarnation/state, bounded queue/in-flight/resource counts and content-free
failure/quarantine codes. It excludes source names, MIME-derived private metadata, text, paths and
provider diagnostic dumps.

### `begin_drain(session, reason) -> DocumentWorkerDrainGuard`

Stops new admission, completes/cancels bounded in-flight requests and enforces a finite drain deadline.

### `shutdown_and_remove(guard, cleanup_policy) -> Result<DocumentWorkerRemovalReceipt, DocWorkerError>`

Drains/cancels, unloads provider/runtime, clears private temp/cache state, closes the channel and verifies
process/child cleanup. Receipt distinguishes graceful, forced verified and incomplete/quarantined
cleanup.

It does not switch Search materialization/projection routes or claim baseline restoration; daemon
integration owns that transition and regression proof.

## Crash and retry semantics

The worker owns no durable Search state. Crash invalidates all unaccepted outputs. Daemon continues
baseline capability, reports optional document degradation and restarts only under the same accepted
profile and bounded policy or quarantines.

Retry requires a new linked attempt or the same canonical operation identity after no terminal product
was accepted. Current-path bytes cannot substitute for the retained revision. The worker never recovers
requests from persistent content state.

## Typed failures

- `OPTIONAL_DEPTH_NOT_ACCEPTED`
- `DOCUMENT_PROVIDER_DISABLED`
- `DOCUMENT_PROVIDER_NOT_QUALIFIED`
- `DOCUMENT_PROFILE_INVALID`
- `DOCUMENT_ARTIFACT_MISMATCH`
- `DOCUMENT_SANDBOX_FAILED`
- `DOCUMENT_PRIVATE_CHANNEL_FAILED`
- `DOCUMENT_PROTOCOL_MISMATCH`
- `DOCUMENT_INPUT_UNSUPPORTED`
- `DOCUMENT_INPUT_MALFORMED`
- `DOCUMENT_INPUT_LIMIT_EXCEEDED`
- `ARCHIVE_MEMBER_POLICY_DENIED`
- `ARCHIVE_BOMB_DETECTED`
- `NO_EXECUTE_POLICY_DENIED`
- `REMOTE_RESOURCE_DENIED`
- `DOCUMENT_RESOURCE_EXHAUSTED`
- `DOCUMENT_DEADLINE_EXCEEDED`
- `DOCUMENT_CANCELLED`
- `DOCUMENT_PROVIDER_CRASHED`
- `DOCUMENT_OUTPUT_INVALID`
- `DOCUMENT_COORDINATE_MAP_INVALID`
- `DOCUMENT_LOSS_MAP_INCOMPLETE`
- `DOCUMENT_CLEANUP_INCOMPLETE`
- `DOCUMENT_WORKER_QUARANTINED`

## Required tests / qualification evidence

- provider/runtime/artifact remains UNSELECTED and feature/binary absent by default;
- accepted P15 + dedicated ADR + exact Windows artifact/license receipt required;
- private daemon-only channel, replay/frame/in-flight bounds and no client endpoint;
- no redb/CAS/Qdrant/source-inventory/handle/secret-store access;
- exact retained revision digest/length and no current-path/Qdrant-payload substitution;
- scripts/JavaScript/VBA/OLE/hooks/filters/shell/child process denied;
- network, external fonts/images/schemas/styles/URLs and credential prompts denied;
- traversal/device/alternate-stream/symlink/hardlink/reparse member denial;
- nested archive/member/decompression/page/object/image/dimension/output bomb budgets;
- malformed/truncated/fuzz corpus cannot crash daemon;
- exact Windows packaging and provider startup fixture;
- UTF-8/CRLF/UTF-16/page/bbox coordinate and loss-map fixtures;
- lossy output never claims exact/high-fidelity assurance;
- finite CPU/memory/commit/disk/temp/output/deadline/queue limits;
- cancellation/timeout/crash cleanup and no publishable unverified temp output;
- source content/path absent from logs/health/crash/receipt surfaces;
- worker removal verifies process/temp/cache cleanup;
- daemon removal returns to accepted P15 text/code materialization behavior;
- public request/response API contains no vendor provider type or executable control.
