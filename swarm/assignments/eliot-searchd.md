# `eliot-searchd` implementation packet

**Path:** `bins/eliot-searchd`  
**Capability:** composition binary  
**Delivery:** W1 shell; progressive through W9  
**Gate:** W1 shell only after W0; each later composition layer requires accepted package receipts from that wave  
**Trace:** S0, S27, S29-S30.3, S32-S33, P01-P15  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-runtime-owner`, `search-os-secrets`, `search-control-redb`, `search-provider-protocol`, progressive accepted capability crates only

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust spelling.

## Mission

Compose accepted capabilities, own the data root and expose the sole local provider/storage/index-process boundary; keep `main` thin.

## Owns

- dependency injection and progressive startup order
- concrete redb, OS-secret, Qdrant supervisor and Qdrant data-plane adapter construction
- wiring concrete adapters to vendor-neutral ports
- bounded scheduler/server lifecycle and readiness/degradation descriptor
- controlled drain, cancellation and shutdown

## Must not own

- reimplementing capability logic in `main`
- reading future-wave dependency docs or enabling unaccepted features
- sharing redb/Qdrant/secret clients with CLI, workers or adapters
- passing concrete vendor clients into query/lifecycle public APIs
- hidden fallback or second data-root owner

## Logical primitives

- `DaemonConfig`, `CompositionProfile`, `StartupPhase`, `ReadinessState`, `CapabilityState`, `DependencyHealth`, `PortBindingSet`, `ShutdownCoordinator`, `CompositionReceipt`

## Logical operations

1. `build_wave1_shell(config) -> Result<Daemon, DaemonError>`
2. `compose_profile(profile, accepted_receipts) -> Result<Daemon, DaemonError>`
3. `construct_adapters(config, owner, secrets) -> Result<AdapterSet, DaemonError>`
4. `bind_ports(capabilities, adapters) -> Result<PortBindingSet, DaemonError>`
5. `start_in_order(owner, journal, source, optional_index, protocol) -> Result<StartupReceipt, DaemonError>`
6. `publish_readiness(states) -> SearchProviderCapabilityDescriptor`
7. `drain_and_shutdown(reason) -> Result<ShutdownReceipt, DaemonError>`

## Required invariants

- startup order is owner → secrets/journal → source → optional Qdrant supervisor → bridge capability gate → request admission
- one daemon owns root, redb and Qdrant lifecycle
- query/lifecycle crates receive ports, never concrete vendor clients
- DIRECT degradation is truthful when indexed profile is unavailable
- later Cargo dependencies are feature-gated by accepted composition wave
- shutdown stops admission, cancels work, expires handles/continuations, releases pins, terminates Qdrant and releases owner in order

## Typed failure surface

- `DAEMON_STARTUP_FAILED`
- `DEPENDENCY_NOT_ACCEPTED`
- `PROFILE_NOT_AVAILABLE`
- `ADAPTER_BINDING_FAILED`
- `QDRANT_UNAVAILABLE`
- `SECURITY_FAIL_CLOSED`
- `SHUTDOWN_INCOMPLETE`

## Exit tests / evidence

- `wave1_shell_builds_without_future_features`
- `second_daemon_denied`
- `startup_order_owner_secrets_journal_source_supervisor_bridge`
- `cli_workers_and_adapters_have_no_store_path`
- `query_packages_receive_ports_not_qdrant_clients`
- `direct_mode_truthful_when_index_down`
- `shutdown_releases_handles_pins_process_and_owner`
- `dependency_graph_has_no_reverse_adapter_edge`

## Suggested internal modules

```text
eliot-searchd/src/
  config.rs
  profile.rs
  compose.rs
  adapters.rs
  ports.rs
  startup.rs
  readiness.rs
  server.rs
  shutdown.rs
  main.rs
```

## Size / split

- Initial `src/` target: **≤ 6,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Keep composition thin; platform/process behavior belongs in the owning library crate.
