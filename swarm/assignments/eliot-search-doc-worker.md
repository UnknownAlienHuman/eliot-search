# `eliot-search-doc-worker` implementation packet

**Path:** `bins/eliot-search-doc-worker`  
**Capability:** optional worker binary  
**Delivery:** W10 / P17  
**Gate:** HARD BLOCK: accepted P15 plus document-provider ADR and qualification receipt required  
**Trace:** S17, S29, P17  
**Direct public handoffs:** `search-contracts`, `search-provider-protocol`, `search-materializer`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Host one accepted document materializer in an isolated no-execute process with bounded IPC and malformed-input isolation.

## Owns

- worker lifecycle and protocol dispatch
- sandbox/resource limits
- qualified provider identity verification
- crash/malformed-input isolation and removal

## Must not own

- provider selection in scaffold
- redb/CAS/Qdrant direct access
- macro, remote-resource or archive execution
- Python/Node production runtime without ADR

## Logical primitives

- DocumentWorkerConfig, ProviderArtifactIdentity, MaterializeDispatch, SandboxPolicy, ResourceLimitSet, WorkerHealth, RemovalReceipt

## Logical operations

1. `verify_provider(config) -> Result<WorkerStartupReceipt, WorkerError>`
2. `serve_materialization(channel, provider, limits) -> Result<(), WorkerError>`
3. `enforce_no_execute_policy(request) -> Result<(), WorkerError>`
4. `cancel(request_id) -> CancelOutcome`
5. `shutdown_and_remove() -> Result<RemovalReceipt, WorkerError>`

## Required invariants

- absent/stopped by default
- provider identity and Windows packaging are qualified before start
- malformed input cannot crash daemon
- no execution/remote fetch/store/index access
- removal returns to baseline text/code materialization

## Typed failure surface

- `OPTIONAL_DEPTH_NOT_ACCEPTED`
- `PROVIDER_NOT_QUALIFIED`
- `NO_EXECUTE_POLICY_DENIED`
- `WORKER_RESOURCE_EXHAUSTED`
- `WORKER_CRASHED`

## Exit tests / evidence

- `feature_absent_by_default`
- `malformed_input_isolation`
- `archive_bomb_budget`
- `macro_and_remote_resource_denied`
- `no_store_index_dependency_guard`
- `provider_removal_test`

## Suggested internal modules

```text
eliot-search-doc-worker/src/
  config.rs
  startup.rs
  dispatch.rs
  sandbox.rs
  limits.rs
  health.rs
  shutdown.rs
  main.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 5,000 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Keep worker thin; materialization semantics remain in search-materializer/provider adapter.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.
