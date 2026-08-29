# `eliot-search-model-worker` implementation packet

**Path:** `bins/eliot-search-model-worker`  
**Capability:** optional worker binary  
**Delivery:** W10 / P16  
**Gate:** HARD BLOCK: accepted P15 plus model-provider ADR and profile receipt required  
**Trace:** S29, P16  
**Direct public handoffs:** `search-contracts`, `search-provider-protocol`, `search-model-provider`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Host an accepted optional model provider in an isolated on-demand process with bounded IPC, resources and cancellation.

## Owns

- worker process lifecycle and protocol dispatch
- resource quotas and cancellation
- artifact/profile identity startup verification
- crash isolation and clean removal

## Must not own

- baseline enablement
- redb/CAS/Qdrant direct access
- model selection before ADR
- canonical decisions or source-content retention beyond policy

## Logical primitives

- WorkerConfig, WorkerStartupReceipt, ModelRequestDispatch, ResourceLimitSet, WorkerHealth, WorkerShutdownReceipt

## Logical operations

1. `verify_profile_and_artifact(config) -> Result<WorkerStartupReceipt, WorkerError>`
2. `serve_requests(channel, provider, limits) -> Result<(), WorkerError>`
3. `cancel(request_id) -> CancelOutcome`
4. `shutdown() -> Result<WorkerShutdownReceipt, WorkerError>`

## Required invariants

- absent/stopped by default
- only daemon-mediated typed requests
- no stores/index clients
- resource and content-retention limits enforced
- removal restores exact P15 behavior

## Typed failure surface

- `OPTIONAL_DEPTH_NOT_ACCEPTED`
- `MODEL_PROVIDER_DISABLED`
- `MODEL_ARTIFACT_MISMATCH`
- `WORKER_RESOURCE_EXHAUSTED`
- `WORKER_CRASHED`

## Exit tests / evidence

- `feature_absent_by_default`
- `no_store_index_dependency_guard`
- `artifact_identity_fixture`
- `cancel_and_resource_limits`
- `crash_isolation`
- `provider_removal_test`

## Suggested internal modules

```text
eliot-search-model-worker/src/
  config.rs
  startup.rs
  dispatch.rs
  limits.rs
  health.rs
  shutdown.rs
  main.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 4,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Keep worker thin; provider behavior belongs in search-model-provider.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.
