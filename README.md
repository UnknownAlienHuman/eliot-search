# ELIOT Search

**Local-first data preparation and retrieval provider. Target: Rust + Qdrant.**

> **Implementation in progress, not a deployment-ready product.** Rust implementations and development
> runtimes exist. Durable redb integration, the canonical DIRECT preparation pipeline and a live Qdrant
> data plane are not yet qualified. See the [Rust/Qdrant audit](docs/audit/ELIOT_SEARCH_AUDIT_2026-09-04.md).
> Architecture and task packets describe requirements; they are not completion evidence.

## Product boundary

ELIOT Search owns source observation, immutable revision readback, preparation, rebuildable retrieval
projections, exact scans, compact candidates, currentness, handles, purge and rebuild. It is not a memory
system, online research service, task controller, canonical knowledge store or client authority service.

The architecture remains:

- Rust product runtime; one daemon owner for each data root.
- Immutable filesystem CAS for retained revisions and preparation artifacts.
- redb for content-free technical control state, never a search index.
- Qdrant as the only indexed retrieval backend. DIRECT must also work without Qdrant.
- Candidates are not evidence: exact source-backed validation remains mandatory.

The normative design is [Architecture 8.4](docs/architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md).
Concrete OS, redb, Qdrant and worker adapters are constructed by the daemon; vendor types, credentials
and reusable authorization decisions must not escape public ports. Paths are locators, not source IDs.

## Actual entrypoints and integration gaps

`cargo run -p eliot-searchd` selects `bins/eliot-searchd/src/entry.rs`.
`cargo run -p eliot-search` selects `bins/eliot-search/src/entry.rs`.

The separate `eliot-search-snapshotd` / `eliot-search-snapshot` binaries are earlier experiments, not the
Qdrant baseline. In particular, the snapshot daemon's local lexical engine must not become a second
product index.

The primary DIRECT store retains and verifies revisions but still searches with the development scanner.
Shared materializer/unitizer code exists; its durable bindings and runtime composition remain unfinished.
`search-control-redb` now includes a concrete `PersistentControlJournal` with real disk transactions,
exact replay/recovery and explicit owner handoff; see [control backend](docs/runtime/CONTROL_REDB.md).
The primary daemon has not yet migrated its file-based control state to that adapter, and the new Rust
tests remain unexecuted. `search-qdrant-bridge` still provides an in-memory model, not a live network adapter.

The primary data-root owner now restores the bounded observation-root catalog under its OS lock.
Explicit registration, listing, unregistering and synchronization commands are documented in
[Source-root registration](docs/runtime/SOURCE_ROOT_REGISTRATION.md). This filesystem registration
adapter is not redb integration, an access grant, a live watcher or a current-workspace proof.
The original audit describes its baseline commit; the earlier roots-not-wired finding is superseded by
this integration. Windows handle-race and power-loss qualification remain outstanding.

Some development/qualification tooling still uses Python. Its migration does not substitute for finishing
the Rust runtime. No new Python product service or alternative index is part of the target architecture.

## Build verification

The repository pins Rust 1.98.0. Run against the exact revision being evaluated:

```sh
cargo +1.98.0 check --workspace --all-targets --all-features --locked
```

The [Manual workspace check](.github/workflows/manual-workspace-check.yml) runs this command once per
explicit dispatch, with a Linux or Windows runner choice. It has read-only repository permissions,
checks the dispatched SHA and never generates source, changes the lockfile or pushes commits.

Focused control-backend, primary-runtime and sealed-store regression tests can be selected with:

```sh
cargo +1.98.0 test -p search-control-redb --lib --locked
cargo +1.98.0 test -p eliot-searchd --bin eliot-searchd --locked
cargo +1.98.0 test -p eliot-searchd --test persistent_roots_process --locked
cargo +1.98.0 test -p eliot-searchd --bin eliot-search-sealed-recover --locked
```

A successful compile is not end-to-end qualification. Windows security needs native Windows tests;
real Qdrant needs live integration tests; restart, crash recovery and exact historical readback must be
verified through the primary daemon. Unexecuted checks remain unexecuted, not accepted.

## Agent and acceptance boundaries

Read [AGENTS.md](AGENTS.md) before changing code. Each package's `FUNCTIONS.md` defines its preconditions,
postconditions, idempotency, bounds and required fixtures. The current Cargo manifests define actual
packages and binary targets; historical scaffold counts must not override them.

Swarm launch authority remains [swarm/launch-state.toml](swarm/launch-state.toml), with package scopes in
`swarm/crates.toml` and `swarm/function-packets.toml`, stage/readset definitions in `swarm/stages.toml`
and `swarm/stage-readsets.toml`, and handoffs under [docs/handoff](docs/handoff/README.md). This README
neither advances a gate nor issues an acceptance receipt. A feature flag, existing source file, model
unit test or commit title cannot do that either.

Configuration ownership is under [config](config/README.md); shared contracts are under
[docs/contracts/p00](docs/contracts/p00/README.md). Plaintext secrets are invalid configuration. Optional
models/documents/advanced-scale features remain subject to their existing explicit qualification gates.
Missing or unavailable evidence cannot authorize them. Never replace an unavailable real backend with an
in-memory model while advertising the real capability, and never invent digests or receipts to bridge APIs.

Exact negative claims require a frozen authoritative denominator, not top-k retrieval. Incomplete
coverage and ambiguity remain explicit. Document workers must not execute source scripts/macros/hooks,
access the network or follow remote resources. Logical deletion is not proof of physical secure erase.

## License

MIT. See [LICENSE](LICENSE).
