# Function contract — `eliot-searchd`

**Status:** progressive W1–W10 composition contract; no daemon business/runtime implementation or
accepted wave receipt exists.

`eliot-searchd` is the sole composition root. It constructs concrete local adapters, owns startup/
readiness/drain/shutdown ordering and exposes the authenticated provider endpoint. Capability meaning
remains in library crates; the daemon must not reimplement source, index, query, security, lifecycle,
evaluation or optional-provider logic.

## Global rules

- one daemon process incarnation owns one data root, control journal and supervised local process set;
- only packages and profiles authorized by exact accepted receipts and compiled features are composed;
- concrete redb, OS-secret, Windows process, Qdrant and worker types remain inside composition adapters;
- public/capability packages receive vendor-neutral ports, never concrete clients/handles;
- readiness and capability descriptors are immutable snapshots coherent with actual handlers, routes,
  configuration and dependency states;
- DIRECT degradation is explicit and remains available when an optional indexed/provider profile is
  unavailable, when its prerequisites permit DIRECT serving;
- cancellation/timeout after a possible durable/external mutation follows the owning package's unknown-
  outcome recovery; daemon orchestration never fabricates rollback;
- normal startup never auto-downloads, upgrades or silently selects artifacts/providers.

## Composition profile operations

### `validate_composition_profile(profile, launch_state, accepted_receipts, compiled_features) -> Result<ValidatedCompositionProfile, DaemonError>`

Requires exact package/wave/gate/config/artifact/API receipt identities. It rejects future-wave package
activation, missing/stale receipts, feature/profile mismatch, multiple root owners, unqualified Qdrant or
optional provider and hidden fallback.

`wave1-shell`, DIRECT, indexed lexical/code, client-edge, evaluation and optional candidates are explicit
closed profiles. A profile may only include dependencies listed in the accepted registry/feature graph.

### `composition_digest(profile) -> CompositionProfileDigest`

Domain-separated canonical digest over enabled package/API/configuration/artifact/route/handler/feature
identities and accepted receipts.

### `classify_profile_change(old, new) -> DaemonCompositionChange`

Returns a composite action plan containing every gate, security barrier, dependency restart, drain,
collection generation/rebuild, route switch/drain/reclaim and capability publication obligation. It does
not collapse unrelated obligations to one severity scalar.

## Configuration acquisition and activation

### `capture_config_inputs(file_bytes, environment, cli, platform) -> Result<CapturedConfigInputs, DaemonError>`

Performs the I/O explicitly outside `search-config`, applies byte/key/value bounds and captures source
provenance. Unknown prefixed environment keys and plaintext-secret-shaped inputs fail closed.

### `build_candidate_config(captured, registry, owners) -> Result<EffectiveConfigCandidate, DaemonError>`

Calls pure `search-config` parsing/layering and each capability owner's typed validation/digest/change
planning. One invalid section rejects the whole candidate.

### `plan_config_activation(current, candidate, dependency_state) -> Result<DaemonReconfigurationPlan, DaemonError>`

Topologically orders composite actions and required receipts while preserving the current authoritative
snapshot. Optional/artifact/profile settings cannot self-authorize.

### `execute_config_activation(plan, ports, context) -> Result<ConfigActivationReceipt, DaemonError>`

Executes only accepted actions through owning package ports. The candidate fingerprint becomes
authoritative in one guarded control/snapshot publication only after every required step/receipt
succeeds.

Cancellation or failure leaves the old snapshot authoritative or the affected capability explicitly
quarantined/fail-closed. Mixed configuration is never published.

### `recover_config_activation(operation, control, dependencies) -> Result<ConfigActivationRecovery, DaemonError>`

Resolves unknown durable/external outcomes by exact owning-package receipts/readback and either finishes
publication, compensates under accepted rules or restores/keeps the old snapshot. It never reruns all
steps blindly.

## W1 startup shell

### `build_wave1_shell(config, launch_state, accepted_w0) -> Result<DaemonBuilder, DaemonError>`

Requires accepted contracts/domain/ports/config handoffs as applicable and constructs no W2+ capability.
It binds private platform adapters only for root ownership, OS secrets, control journal and provider
protocol.

### `acquire_root(builder, runtime_owner, context) -> Result<OwnedDaemonBuilder, DaemonError>`

Acquires the exact data-root owner guard before opening secrets/journal or endpoints. A conflicting or
ambiguous owner blocks/quarantines startup.

### `initialize_secrets_and_control(builder, secret_port, control_port, context) -> Result<ControlledDaemonBuilder, DaemonError>`

Validates the OS-secret backend, obtains only purpose-bound leases and opens/migrates/verifies the exact
control journal under the owner guard. Plaintext never enters daemon config, argv, logging or receipts.

### `publish_control_snapshot(builder, snapshot_port, context) -> Result<SnapshotDaemonBuilder, DaemonError>`

Reconstructs and publishes the immutable committed control snapshot before request admission. A
committed-but-unpublished state remains fail-closed and recoverable.

### `start_provider_endpoint(builder, protocol_port, context) -> Result<Wave1Daemon, DaemonError>`

Starts the local mutually authenticated provider endpoint with frame/in-flight/backpressure ceilings.
Wave1 exposes only shell/admin/health capabilities whose handlers actually exist; no source/query/index
recipe is advertised.

## Progressive capability composition

### `compose_source_spine(daemon, accepted_w2, ports, context) -> Result<DirectDaemon, DaemonError>`

Binds admitted root/identity/registry/safe-reader/revision/materializer/unitizer ports after exact W2
handoffs. It does not construct Qdrant or report indexed capability.

### `compose_index_profile(daemon, accepted_w3, artifact_receipts, ports, context) -> Result<IndexedDaemon, DaemonError>`

Starts the exact qualified Qdrant process, connects the qualified data plane, validates collection/
lexical/profile/publication/pin/reclaim receipts and publishes indexed capability only after coherent
route/control/handler state.

Qdrant health alone is not profile readiness. Failure may truthfully retain DIRECT mode.

### `compose_query_profile(daemon, accepted_w4, ports, context) -> Result<QueryDaemon, DaemonError>`

Binds access/planner/executor/validator/handles/results/continuations/eval through vendor-neutral ports and
publishes only recipes with live handlers and accepted capability requirements.

### `compose_currentness_and_proof(daemon, accepted_w5_w6_w7, ports, context) -> Result<HardenedDaemon, DaemonError>`

Progressively adds reconciliation/overlay/code structure, resolution/comparison/exact proof and
lifecycle/security hardening without bypassing earlier source/access/publication owners.

### `compose_client_edge(daemon, accepted_w8, ports, context) -> Result<ClientDaemon, DaemonError>`

Publishes authenticated generic client/CLI and separately gated optional leaf mappings. Binding-filtered
capabilities never grant scope/authority.

### `compose_evaluation_mode(daemon, accepted_w9, ports, context) -> Result<EvaluationCapableDaemon, DaemonError>`

Adds explicit authenticated qualification mode only. It is disabled during normal startup and exposes no
direct store bypass or optional depth.

### `compose_optional_candidate(daemon, accepted_w10_candidate, ports, context) -> Result<OptionalDaemon, DaemonError>`

Requires exact accepted P15, candidate ADR/artifacts/profile/benefit/removal/migration receipts. One
candidate is staged/validated/committed through the W10 integration contract; package/config/worker
presence alone cannot activate it.

## Concrete adapter construction

### `construct_platform_adapters(profile, config, owner, secret_leases) -> Result<PrivateAdapterSet, DaemonError>`

Constructs only adapters required by the accepted profile. Vendor/native handles are private,
non-serializable and destroyed after owning services. Secret leases are purpose/incarnation-bound.

### `bind_ports(capabilities, adapters) -> Result<PortBindingSet, DaemonError>`

Checks one implementation per required port, matching API/profile/incarnation, no unbound mandatory port
and no reverse concrete-adapter dependency. Duplicate/conflicting bindings reject.

### `verify_dependency_graph(bindings, profile) -> Result<DependencyGraphReceipt, DaemonError>`

Proves package-to-port direction, startup/shutdown topological order and absence of CLI/worker/leaf direct
store paths.

## Readiness and capability publication

### `collect_dependency_health(services, deadline, cancel) -> DependencyHealthSnapshot`

Collects bounded content-free health/state identities. Missing/unavailable health is explicit and does
not automatically become ready.

### `derive_readiness(profile, config, control, routes, handlers, health, accepted_receipts) -> ReadinessState`

Purely derives:

```text
STARTING
DIRECT_READY
INDEXED_READY
DEGRADED
DRAINING
QUARANTINED
STOPPED
```

with exact capability exclusions/reasons. It cannot infer currentness from Qdrant or process liveness.

### `publish_capability_snapshot(readiness, binding_policy, snapshot_port, operation) -> Result<CapabilityPublishReceipt, DaemonError>`

Publishes one immutable coherent config/handler/route/capability descriptor. For every recipe/profile:

```text
descriptor says available ⇔ handler exists and all accepted prerequisites are live
```

Binding filtering narrows disclosure/capability but never grants access. Lost receipt is recovered by
snapshot generation/digest readback.

## Request admission and lifecycle

### `admit_connection(endpoint, protocol, binding_policy, limits) -> Result<ConnectionGuard, DaemonError>`

Requires authenticated paired local binding, exact incarnation/session sequence and finite connection/
in-flight/frame limits. It exposes no concrete adapter/store handle.

### `admit_request(connection, request, snapshot, budget, cancel) -> Result<RequestGuard, DaemonError>`

Validates protocol/request identity, binding/grant, available recipe/handler, relative deadline and
resource budget against one immutable capability/control snapshot. Hot read admission performs no redb
write.

### `dispatch_request(guard, handler_registry, context) -> Result<TerminalResult, DaemonError>`

Routes to exactly one accepted server-owned handler. Progress is monotonic and at most one terminal
result is emitted. The daemon does not reinterpret partial/degraded/ambiguous/incomplete package outputs.

### `cancel_request(connection, request_id) -> CancelOutcome`

Idempotently signals request-local work and releases request resources/pins/leases according to owning
ports. Cancellation after possible mutation follows the mutation owner's recovery semantics.

### `handle_disconnect(connection) -> DisconnectCleanupReceipt`

Stops emission and releases request-local handles, continuations, pins, windows and quotas. Durable
commands continue/recover under their operation identities; ephemeral state never silently persists.

## Recovery

### `recover_startup(config, launch_state, owner_observation, control_observation, process_observation) -> Result<StartupRecoveryPlan, DaemonError>`

Orders root-owner recovery, secrets/control verification, committed snapshot reconstruction, supervised
process identity and capability state. Ambiguity quarantines; a responding endpoint/collection does not
prove ownership/control.

### `recover_operations(control, operation_registry, ports, context) -> Result<OperationRecoveryReceipt, DaemonError>`

Delegates every unresolved mutation to its owning package and publishes coherent control/capability state
after exact outcomes. It never invents a shared generic rollback rule.

## Drain and shutdown

### `begin_drain(daemon, reason, operation) -> Result<DaemonDrainGuard, DaemonError>`

Publishes draining state, stops new connection/request admission and captures exact active work/service
sets.

### `drain_requests(guard, deadline, cancellation_policy) -> Result<RequestDrainReceipt, DaemonError>`

Completes/cancels bounded requests and verifies handle/continuation/pin/window/quota cleanup. Unresolved
durable mutation remains explicit and blocks unsafe owner release.

### `shutdown_services(guard, services, deadline) -> Result<ServiceShutdownReceipt, DaemonError>`

Stops in reverse dependency order: optional workers/routes, query/index/source services, Qdrant bridge/
process, provider endpoint, control/secrets. Every service supplies its own shutdown/cleanup receipt.

### `release_owner(guard, service_receipts, runtime_owner, deadline) -> Result<DaemonShutdownReceipt, DaemonError>`

Releases the data-root owner only after exact dependency receipts prove no live service still requires
it. Unknown cleanup/release state quarantines rather than reports clean shutdown.

## Cancellation, deadline and crash semantics

Startup/composition/reconfiguration/shutdown steps use stable operation identities and finite deadlines.
Pure planning may retry freely. After any possible external/durable mutation, exact owner/package
readback determines state. Daemon crash invalidates process-local sessions/pins/leases and recovery begins
from owner/control/operation records; it never assumes the last in-memory phase completed.

## Typed failures

- `DAEMON_PROFILE_INVALID`
- `DAEMON_DEPENDENCY_NOT_ACCEPTED`
- `DAEMON_FEATURE_NOT_COMPILED`
- `DAEMON_STARTUP_FAILED`
- `DAEMON_STARTUP_OUTCOME_UNKNOWN`
- `DAEMON_OWNER_UNAVAILABLE`
- `DAEMON_CONTROL_UNAVAILABLE`
- `DAEMON_ADAPTER_BINDING_FAILED`
- `DAEMON_CAPABILITY_INCOHERENT`
- `DAEMON_CONFIG_ACTIVATION_FAILED`
- `DAEMON_CONFIG_OUTCOME_UNKNOWN`
- `DAEMON_PROFILE_NOT_AVAILABLE`
- `DAEMON_REQUEST_UNAUTHORIZED`
- `DAEMON_RESOURCE_EXHAUSTED`
- `DAEMON_REQUEST_CANCELLED`
- `DAEMON_DEPENDENCY_DEGRADED`
- `DAEMON_SECURITY_FAIL_CLOSED`
- `DAEMON_QUARANTINED`
- `DAEMON_SHUTDOWN_INCOMPLETE`
- `DAEMON_OWNER_RELEASE_BLOCKED`

## Required tests / qualification evidence

- wave1 shell compiles/composes without W2+ feature initialization;
- startup order owner → secrets/control/snapshot → source → Qdrant/bridge → request admission;
- crash/reopen at every startup/config/endpoint/capability/shutdown boundary;
- second daemon/root owner denied and PID/port identity never sufficient;
- plaintext secrets absent from config/argv/env/log/error/receipt/process launch;
- control committed snapshot precedes readiness/admission;
- exact function/handler/capability/config/route coherence;
- DIRECT truthful degradation when index/optional provider unavailable;
- no concrete vendor/store type in capability/query/lifecycle public boundaries;
- CLI/workers/leaf adapters have no direct store path;
- hot read admission creates zero durable control writes;
- config composite obligations preserved and failed activation keeps old fingerprint;
- request frame/in-flight/budget/backpressure/cancel/disconnect cleanup;
- exactly one terminal result and typed partial/degraded preservation;
- unknown mutation outcomes delegated to exact owning-package recovery;
- reverse-order shutdown and owner release only after all receipts;
- optional/evaluation profiles disabled without exact gates;
- fake ports test composition logic independently from Windows/redb/Qdrant/provider runtimes;
- dependency graph, feature gate and hand-written line-budget guards.
