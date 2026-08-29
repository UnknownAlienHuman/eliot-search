# ELIOT Search

**Local-first data preparation and retrieval provider for ELIOT Memory OS.**

> **Status: Architecture 8.4 published; exact P00 contract pack and 44-package swarm scaffold; no
> business implementation.** Runtime correctness, performance, security execution, migration and
> product acceptance remain unproven.

## Product boundary

ELIOT Search owns local source observation, immutable revision readback, preparation, rebuildable
retrieval projections, exact scans, compact candidate results, currentness, handles, purge and rebuild.
It is not a memory system, online research service, task controller, canonical knowledge store or
authority service.

## Foundation

```text
search-contracts   shared IDs, wire/domain records and reason registries
     ├─ search-domain   pure transition, ordering, eligibility and coverage meaning
     └─ search-ports    shared vendor-neutral trait boundary and conformance interfaces
```

The P00 implementation projection is under [`docs/contracts/p00/`](docs/contracts/p00/README.md). It
contains field-level schemas, exact recipe families, canonical encoding, reason namespaces and port
operations so W0 agents do not reconstruct contracts from the 145 KB master.

## Swarm decomposition

The workspace contains **40 library packages and 4 binaries**. One writer owns one Cargo package. A
package targets at most 7,500 `src/` lines unless its assignment sets a lower target; split review is
mandatory before 8,500 total hand-written lines and 10,000 is a hard stop.

The five security/lifecycle support packages remain:

```text
search-os-secrets
search-source-admission
search-qdrant-supervisor
search-index-reclaimer
search-handles
```

Family directories are navigation only. Exact paths, dependencies, assignments and wave metadata are
in [`swarm/crates.toml`](swarm/crates.toml) and
[`docs/handoff/CRATE_MATRIX.md`](docs/handoff/CRATE_MATRIX.md).

## Launch gate

[`swarm/launch-state.toml`](swarm/launch-state.toml) is the only implementation authority.

Current W0 order:

```text
1. search-contracts
2. after accepted contracts handoff/API digest:
   - search-domain
   - search-ports
3. integration owner publishes W0 receipt
```

Every W1+ package remains blocked. Optional model/document depth additionally requires accepted P15,
a dedicated ADR and exact provider qualification.

## Port rule

Capability/orchestration packages consume `search-ports`. Concrete redb, OS-secret, Qdrant process and
Qdrant data-plane implementations are constructed only by `eliot-searchd`. Vendor/native types,
credentials, raw collections, point IDs and reusable authorization decisions never cross public ports.

## Honest status

`Cargo.toml` files and empty `src/lib.rs` / `src/main.rs` establish package boundaries only. P00 still
must implement contracts/domain/ports, pin the actual Windows-compatible Rust/dependency set, generate
`Cargo.lock` and execute the real policy/test suite.

## License

MIT. See [LICENSE](LICENSE).
