# `eliot-searchd` implementation packet

**Path:** `bins/eliot-searchd`  
**Capability:** composition binary  
**Delivery:** W1 shell; progressive through W9  
**Gate:** W1 shell only after W0; each later composition layer requires accepted package receipts from that wave  
**Trace:** S0, S27, S29-S30.3, S32-S33, P01-P15  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-runtime-owner`, `search-control-redb`, `search-provider-protocol`, `progressive accepted capability crates only`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Compose accepted capabilities, own the data root and expose the sole local provider/storage process boundary; keep main thin.

## Owns

- composition root, dependency injection and startup order
- sole qdrant process supervision/credentials through accepted adapters
- bounded scheduler/server lifecycle and readiness/degradation descriptor
- controlled drain, cancellation and shutdown

## Must not own

- reimplementing capability logic in main
- reading future-wave dependency docs or enabling unaccepted features
- sharing redb/Qdrant clients with CLI/workers/adapters
- hidden fallback or second data-root owner

## Logical primitives

- DaemonConfig, CompositionProfile, StartupPhase, ReadinessState, CapabilityState, DependencyHealth, ShutdownCoordinator, CompositionReceipt

## Logical operations

1. `build_wave1_shell(config) -> Result<Daemon, DaemonError>`
2. `compose_profile(profile, accepted_receipts) -> Result<Daemon, DaemonError>`
3. `start_in_order(owner, journal, source, optional_index, protocol) -> Result<StartupReceipt, DaemonError>`
4. `publish_readiness(states) -> SearchProviderCapabilityDescriptor`
5. `drain_and_shutdown(reason) -> Result<ShutdownReceipt, DaemonError>`

## Required invariants

- startup order is owner then journal/source then optional Qdrant then request admission
- one daemon owns root, redb and qdrant lifecycle
- DIRECT degradation is truthful when indexed profile is unavailable
- later Cargo dependencies are feature-gated by composition wave
- shutdown cancels work and releases request pins before owner release

## Typed failure surface

- `DAEMON_STARTUP_FAILED`
- `DEPENDENCY_NOT_ACCEPTED`
- `PROFILE_NOT_AVAILABLE`
- `QDRANT_UNAVAILABLE`
- `SECURITY_FAIL_CLOSED`
- `SHUTDOWN_INCOMPLETE`

## Exit tests / evidence

- `wave1_shell_builds_without_future_features`
- `second_daemon_denied`
- `startup_order_fixture`
- `cli_and_workers_have_no_store_path`
- `direct_mode_truthful_when_index_down`
- `shutdown_releases_pins_and_owner`

## Suggested internal modules

```text
eliot-searchd/src/
  config.rs
  profile.rs
  compose.rs
  startup.rs
  supervisor.rs
  readiness.rs
  server.rs
  shutdown.rs
  main.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 6,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Keep main/composition under 6,500 lines. Runtime behavior that grows belongs in an owning library crate, never another composition facade.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.
