# ELIOT Search

Local-first data preparation and retrieval. **Target architecture: Rust + Qdrant.**
Implementation is incomplete; this is not a qualified production release.

## Runtime

The supported entrypoints are `eliot-searchd` and `eliot-search`, both built from
`src/entry.rs` in their respective packages. The six sealed prototypes and two
snapshot programs are now test-harness targets, not installable product binaries
or runnable examples. Their source and regression tests remain in all-target checks.
The snapshot/BM25 experiment is not an alternative product index.

The primary DIRECT runtime retains and verifies source revisions, then uses the
shared UTF-8 materializer, unitizer and cross-unit literal matcher. Windows writes
protect and verify new revisions before source publication. Existing SHA-256
identities are not relabeled as BLAKE3 or replaced by fabricated receipts.

Persistent root registration is connected. Missing catalog state with retained
objects is rejected rather than recreated as an empty corpus. A damaged proxy
exchange is not reused for another client. The primary service now stops after
an uncertain mutation or failed response, discards handles and refuses queued
commands; invalid or oversized frames also terminate the session.

The main unfinished integrations are **primary control-state migration to redb,
durable canonical preparation manifests, and the real Qdrant data plane**.
`PersistentControlJournal` already performs real redb I/O, but the primary source
catalog has not switched to it. The Qdrant bridge remains an in-memory model.
Preparation is currently recomputed at query time. Native Windows security,
full ownership/currentness/access/lifecycle behavior and release qualification
still require the corresponding task implementations and executed tests.

## Build and tests

Rust is pinned to 1.98.0. Build the main executables explicitly:

```sh
cargo +1.98.0 build --release --locked -p eliot-searchd -p eliot-search --bins
cargo +1.98.0 check --workspace --all-targets --all-features --locked
cargo +1.98.0 test --workspace --all-targets --all-features --locked
```

Focused tests:

```sh
cargo +1.98.0 test --locked -p search-control-redb --lib
cargo +1.98.0 test --locked -p search-materializer -p search-unitizer -p search-exact --lib
cargo +1.98.0 test --locked -p eliot-searchd --bin eliot-searchd --test persistent_roots_process --test direct_preparation_process --test catalog_loss_process --test service_failure_process --test product_targets
cargo +1.98.0 test --locked -p eliot-searchd --test eliot-search-sealed-recover
```

[Manual workspace check](.github/workflows/manual-workspace-check.yml) remains
`workflow_dispatch` only, read-only and exact-SHA. `core_tests` runs the primary
regressions and all eight retained legacy harnesses. Native DPAPI tests require
the Windows runner; they are not validated by a Linux run. No fresh passing
Rust build or test run is claimed by this change.

## Boundaries and documentation

Qdrant is the only indexed retrieval backend. DIRECT works independently of it.
redb stores technical control state, never searchable content. Immutable revisions
and derived artifacts belong in scoped CAS. Candidates require source-backed
validation; handles never grant access. Exact negative claims require a frozen
source denominator, not top-k results. Logical deletion is not physical erasure.
Optional models/documents stay behind their explicit qualification gates.

Read [AGENTS.md](AGENTS.md) and package `FUNCTIONS.md` before changing code. The
[normative architecture](docs/architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md)
remains authoritative. Existing task PRs track completion; landing code in `main`
does not fabricate accepted gates or independent review.

[DIRECT smoke commands](QUICKSTART.md) ·
[service fail-stop and target isolation](docs/runtime/SERVICE_FAIL_STOP.md) ·
[root registration](docs/runtime/SOURCE_ROOT_REGISTRATION.md) ·
[revision writes](docs/runtime/DIRECT_REVISION_WRITES.md) ·
[DIRECT preparation](docs/runtime/DIRECT_PREPARATION.md) ·
[redb adapter](docs/runtime/CONTROL_REDB.md) ·
[catalog/proxy guards](docs/runtime/CATALOG_LOSS_AND_CHANNEL_FAILURE.md)

Python remains in some development validators; no Python product service is part
of the architecture. Migrating that tooling does not replace finishing the runtime.

MIT. See [LICENSE](LICENSE).
