# ELIOT Search

**Local-first data preparation and retrieval provider for ELIOT Memory OS.**

> **Status: Architecture 8.4 published; exact P00 contract pack and 45-package swarm scaffold; no
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
     ├─ search-ports    shared vendor-neutral trait boundary and conformance interfaces
     └─ search-config   pure configuration layering, redaction and reconfiguration planning
```

The P00 implementation projection is under [`docs/contracts/p00/`](docs/contracts/p00/README.md).
Configuration ownership is under [`config/`](config/README.md). Registry-declared package
`FUNCTIONS.md` files specify operation preconditions, postconditions, idempotency, cancellation,
unknown-outcome recovery, resource bounds and required fixtures.

## Swarm decomposition

The workspace contains **41 library packages and 4 binaries**. One writer owns one Cargo package. A
package targets at most 7,500 `src/` lines unless its assignment sets a lower target; split review is
mandatory before 8,500 total hand-written lines and 10,000 is a hard stop.

The machine authority is [`swarm/crates.toml`](swarm/crates.toml). It records exact Cargo dependencies,
assignments, function packets, configuration sections and qualification inputs. The human index is
[`docs/handoff/CRATE_MATRIX.md`](docs/handoff/CRATE_MATRIX.md).

## Configuration

`search-config` owns only deterministic parsing/layering, provenance, redaction, fingerprints, diffs
and reconfiguration planning. Capability packages own their typed sections and runtime application.
Plaintext secrets are invalid; only opaque secret references may appear in configuration.

The example file is safe DIRECT mode. Indexed W3 settings remain disabled or `UNQUALIFIED` until an
exact Qdrant server/client pair, lexical profile and capability suite are accepted.

## W3 qualification

[`qualification/qdrant/W3_QUALIFICATION.md`](qualification/qdrant/W3_QUALIFICATION.md) is the
architecture-to-evidence contract for the Qdrant process, data plane, lexical profiles, collection
schema, publication, pins and ordinary exact reclaim.

No release tag or version string is accepted by itself. Failure or unavailable mandatory evidence keeps
indexed mode disabled while DIRECT/exact operation may remain truthfully degraded.

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

Every W1+ package, including `search-config`, remains blocked. Optional model/document depth
additionally requires accepted P15, a dedicated ADR and exact provider qualification.

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
