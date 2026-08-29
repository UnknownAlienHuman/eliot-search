# W10 optional-depth daemon integration

**Status:** integration contract only; P16-P18/G6 remains blocked.  
**Owner:** `eliot-searchd` composition root.  
**Packages remain independent:** model semantics belong to `search-model-provider`; worker runtime to the
worker binaries; scale data plane/publication/pins/reclaim to their existing owners.

## Gate evaluation

### `evaluate_optional_candidate(candidate, control_snapshot, receipts) -> OptionalCandidateEligibility`

Requires exact accepted P15 report/reviewer receipt, dedicated candidate ADR, exact profile/artifact and
Windows qualification, compiled Cargo feature, explicit disabled-by-default config, current binding
policy, measured benefit, removal plan and migration/rollback evidence or reviewed not-applicable proof.

Returns one closed state:

```text
INELIGIBLE
ELIGIBLE_FOR_QUALIFICATION
QUALIFIED_NOT_MEASURED
CANDIDATE_ACCEPTED_NOT_STAGED
STAGED_NOT_SERVING
ACTIVE
DRAINING
QUARANTINED
```

Configuration or feature presence alone never returns an eligible/active state.

### `verify_candidate_receipt_chain(candidate, receipts) -> Result<VerifiedCandidateReceipts, OptionalDepthError>`

Binds every receipt to the same P15 baseline, repository/API/configuration identities, candidate profile,
artifacts, Windows environment and independent reviewer. Stale, mixed, self-reviewed or mutable evidence
fails closed.

## Worker composition

### `start_model_worker(candidate, process_port, secret_port, context) -> Result<ModelWorkerGuard, OptionalDepthError>`

Starts only the exact qualified optional binary/profile under private authenticated IPC, Windows
containment and finite resource policy. The daemon passes a bounded purpose/incarnation-bound channel
credential; plaintext never enters argv, logs or public config.

Worker readiness does not activate semantic capability. Startup ambiguity is resolved by exact process,
binary, profile, channel and parent/owner identity, not a responding endpoint.

### `start_document_worker(candidate, process_port, secret_port, context) -> Result<DocumentWorkerGuard, OptionalDepthError>`

Applies the same identity/containment rules plus the exact no-execute/no-network sandbox and private temp
policy. It does not grant access to Search stores or client endpoints.

### `monitor_optional_worker(guard, policy) -> OptionalWorkerDecision`

Classifies healthy, degraded, bounded restart, drain, quarantine or removal. Restart never switches
artifact/profile, downloads an update or expands resource/content policy. Repeated/crash/identity/sandbox
failure ends in quarantine while P15 baseline remains serviceable.

## Candidate preparation

### `plan_optional_activation(candidate, current, ports) -> Result<OptionalActivationPlan, OptionalDepthError>`

Constructs a composite plan with all required obligations:

```text
GATE_REQUIRED
WORKER_START_OR_RESTART
SECURITY_BARRIER
NEW_COLLECTION_GENERATION / REBUILD_PROJECTION when applicable
CANDIDATE_VALIDATION
GUARDED_CONFIG_ROUTE_COMMIT
CAPABILITY_SNAPSHOT_PUBLICATION
OLD_ROUTE_DRAIN_AND_RECLAIM when replacing/removing
```

`RERANK_ONLY` has no persistent-vector generation but still requires gate, worker, handler/config,
benefit and removal steps. Dense/multivector/document/scale candidates require generation/migration.

No scalar dominant action may erase another obligation.

### `stage_optional_candidate(plan, workers, ports, context) -> Result<OptionalStageReceipt, OptionalDepthError>`

Starts qualified workers, creates only candidate-local routes/profile state and prepares exact manifests.
It never changes the visible serving route or authoritative effective config. Candidate failure is
contained and discarded/quarantined.

### `build_candidate_generation(plan, preparation_ports, publication_ports, context) -> Result<CandidateGenerationReceipt, OptionalDepthError>`

For dense/multivector/document/scale candidates, creates an incompatible new collection generation,
builds a baseline at R0, catches ordered changes up and enters the final barrier. It uses existing
publication/migration owners through ports; the daemon does not duplicate their state machines.

### `validate_optional_candidate(plan, candidate_state, eval_receipts, ports, context) -> Result<OptionalCandidateValidation, OptionalDepthError>`

Requires exact schema/profile/point/readback, access/currentness/IDF/scoring noninterference, fault and
resource evidence, accepted incremental-benefit receipt, worker/content-policy audits and rollback/removal
readiness. Validation cannot consume hidden content or reinterpret an unavailable probe as pass.

## Activation and capability publication

### `commit_optional_activation(validated, control_port, current_snapshot, mutation) -> Result<OptionalActivationCommit, OptionalDepthError>`

One guarded control transaction rechecks P15/gate/profile/artifact, source/access/shadow/purge, worker,
route/generation and configuration generations and atomically commits the selected optional profile,
serving route/handler and effective config fingerprint.

Qdrant alias changes, worker readiness, candidate build or configuration parse are not activation
linearization points.

Cancellation after possible control commit is resolved through operation/control readback. The daemon
never reports rollback until committed state is known.

### `publish_optional_capability_snapshot(commit, snapshot_port, context) -> Result<CapabilityPublishReceipt, OptionalDepthError>`

Publishes one coherent immutable handler/capability/config/route snapshot. A binding-filtered descriptor
reports optional capability available only after the matching handler, worker, profile, route and gate
receipts are active.

Failure after control commit enters fail-closed recovery; it cannot expose a handler/descriptor mismatch.

### `authorize_optional_request(binding, grant, capability, request) -> Result<OptionalRequestPermit, OptionalDepthError>`

Requires current binding/profile authorization and intersects requested use with server policy. Optional
availability grants no new corpus/source/disclosure authority. A stale descriptor never widens access.

## Failure and degradation

### `classify_optional_failure(operation, worker, route, baseline) -> OptionalFailureDecision`

Returns one explicit action:

```text
RETRY_OPTIONAL_WITHIN_BOUND
DROP_OPTIONAL_LEG_WITH_GAP
PRESERVE_PRE_RERANK_ORDER_WITH_GAP
DRAIN_AND_DISABLE_OPTIONAL
QUARANTINE_OPTIONAL
FAIL_REQUEST
```

The accepted P15 baseline does not depend on the candidate. No hidden provider fallback or automatic
profile switch occurs.

### `recover_optional_activation(operation, control, routes, workers, context) -> Result<OptionalRecoveryReceipt, OptionalDepthError>`

Uses durable control state, exact route/profile readback and worker identity to finish activation,
reconstruct the receipt, restore the prior baseline or quarantine. It never guesses from a live process
or collection and never reuses a migration generation/operation under different input.

## Deactivation and removal

### `plan_optional_removal(candidate, current, reason) -> Result<OptionalRemovalPlan, OptionalDepthError>`

Produces exact steps for capability drain, baseline route/config restoration, in-flight cancellation,
route-pin drain, worker shutdown/cache cleanup, optional manifest reclaim and P15 regression validation.

### `commit_baseline_restore(plan, control_port, mutation) -> Result<BaselineRestoreCommit, OptionalDepthError>`

Atomically returns new requests to the accepted P15 handler/profile/route/configuration. It does not wait
for physical optional-route deletion. Restrictive/security state is rechecked inside the transaction.

### `drain_and_remove_optional(plan, pins, workers, reclaimer, context) -> Result<OptionalRemovalReceipt, OptionalDepthError>`

Publishes draining/unavailable capability, releases/cancels bounded optional work, waits for route/pin
watermarks, stops the worker, clears optional input/temp/cache state and exact-reclaims optional routes
when safe. Deferred reclaim remains explicit.

Success requires a P15 regression fixture and content-minimized receipt. It never claims secure erase
without evidence.

## P18 advanced-scale integration

### `plan_scale_candidate(profile, bottleneck, current_route) -> Result<ScaleMigrationPlan, OptionalDepthError>`

Requires an accepted measured one-shard bottleneck and dedicated ADR. It delegates topology/schema
qualification to the bridge and migration/recovery to publication. The plan binds R0/R1, change-log,
barrier, exact manifests, route/config guards, pin-drain and rollback policy.

### `commit_scale_route_switch(validated, control, mutation) -> Result<ScaleRouteCommit, OptionalDepthError>`

Uses the same guarded redb route linearization. The candidate must satisfy access/currentness/scoring/IDF
and fault equivalence or declare a distinct accepted scoring/product profile. An alias is not commit.

### `rollback_scale_route(plan, control, ports, context) -> Result<ScaleRollbackReceipt, OptionalDepthError>`

Before route switch, exact-discards the candidate. After switch, builds/validates or retains a baseline
route and performs another guarded forward route transition; it never rewinds epochs or mutates active
schema in place.

## Configuration

Owns `config/sections/optional_profiles.md`. Compiled defaults keep semantic/document/advanced-scale
false and profiles absent. Section validation accepts only opaque qualified profile refs and cannot
resolve artifacts or gate receipts.

`plan_section_change` emits composite obligations. `apply_live_change` cannot activate/deactivate an
optional profile because all such transitions require gate, worker and potentially route/config control
receipts.

## Typed failures

- `OPTIONAL_DEPTH_NOT_ACCEPTED`
- `OPTIONAL_CANDIDATE_ADR_MISSING`
- `OPTIONAL_PROFILE_DISABLED`
- `OPTIONAL_PROFILE_NOT_QUALIFIED`
- `OPTIONAL_RECEIPT_CHAIN_MISMATCH`
- `OPTIONAL_FEATURE_NOT_COMPILED`
- `OPTIONAL_BINDING_NOT_AUTHORIZED`
- `OPTIONAL_WORKER_START_FAILED`
- `OPTIONAL_WORKER_IDENTITY_MISMATCH`
- `OPTIONAL_WORKER_QUARANTINED`
- `OPTIONAL_CANDIDATE_BUILD_FAILED`
- `OPTIONAL_CANDIDATE_VALIDATION_FAILED`
- `OPTIONAL_BENEFIT_NOT_PROVED`
- `OPTIONAL_ACTIVATION_GUARD_CHANGED`
- `OPTIONAL_ACTIVATION_OUTCOME_UNKNOWN`
- `OPTIONAL_CAPABILITY_PUBLICATION_FAILED`
- `OPTIONAL_REMOVAL_INCOMPLETE`
- `P15_BASELINE_RESTORE_FAILED`
- `SCALE_BOTTLENECK_NOT_PROVED`
- `SCALE_MIGRATION_BLOCKED`
- `SCALE_ROLLBACK_FAILED`

## Required tests / evidence

- no candidate active without exact P15/ADR/artifact/profile/benefit/removal/migration chain;
- feature/config alone never activates optional capability;
- handler/descriptor/config/route snapshot is coherent;
- provider failure leaves P15 baseline available with explicit optional gap;
- dense/multivector/document/scale candidates never mutate active collection in place;
- activation crash at every worker/build/validation/control/snapshot boundary;
- stale access/purge/shadow/profile/worker guard rejects activation;
- capability is binding-filtered and grants no authority;
- worker identity/restart/quarantine and no plaintext channel secret;
- rerank-only not-applicable migration receipt;
- exact route migration/readback/final barrier and old-route pin drainage;
- deactivation restores P15 route/config before optional physical reclaim;
- worker/cache/temp cleanup and exact/deferred manifest receipt;
- accepted P15 regression after removal;
- advanced scale requires measured bottleneck, equivalence and rollback;
- no automatic workflow, download, update, provider switch or self-acceptance.
