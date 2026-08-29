# ELIOT Search

**Local-first data preparation and retrieval provider for ELIOT Memory OS.**

> **Status: Architecture 8.4 published; exact P00 contract pack and 45-package swarm scaffold; W3–W6
> bounded implementation/qualification packets; no business implementation.** Runtime correctness,
> performance, security execution, migration and product acceptance remain unproven.

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

## Prepared product slices

- **W3 / P05–P07:** lexical profiles, qualified Qdrant process/data plane, exact projection manifests,
  serialized publication, epoch/route pins and ordinary exact reclaim.
- **W4 / P08:** pre-candidate access, bounded query planning/execution, source-backed validation,
  handles, compact result projection, continuation and protocol/evaluation contracts.
- **W5 / P09–P10:** observation-gap reconciliation, truthful current-workspace preflight,
  saved/unsaved overlay precedence and one qualified no-execute Rust tolerant-syntax profile.
- **W6 / P11–P12:** ambiguity-preserving subject resolution, descriptive lineage/configuration-aware
  comparison and frozen-denominator exact proof.

W6 package ownership and hard stops are indexed in
[`docs/handoff/W6_IMPLEMENTATION_PACKET.md`](docs/handoff/W6_IMPLEMENTATION_PACKET.md). Its machine
evidence contract is [`qualification/proof/`](qualification/proof/README.md). All 52 probes begin
`UNAVAILABLE`; no regex engine, structural exact profile, resolution policy or comparison policy is
selected by the scaffold.

## Configuration

`search-config` owns only deterministic parsing/layering, provenance, redaction, fingerprints, diffs
and reconfiguration planning. Capability packages own their typed sections and runtime application.
Plaintext secrets are invalid; only opaque secret references may appear in configuration.

The example file is safe DIRECT mode. Indexed W3 settings remain disabled or `UNQUALIFIED` until an
exact Qdrant server/client pair, lexical profile and capability suite are accepted.

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

Every W1+ package, including `search-config`, remains blocked. Prepared W3–W6 packets are future bounded
inputs only. Optional model/document depth additionally requires accepted P15, a dedicated ADR and
exact provider qualification.

## Non-overclaim rules

- material subject ambiguity is returned, not guessed away;
- comparison is descriptive and never chooses a correct/best implementation;
- forks/mirrors do not inflate independent evidence;
- exact negative proof uses an authoritative frozen inventory denominator, never Qdrant/top-k;
- unreadable, drifted, cancelled, timed-out or otherwise incomplete items block complete-negative claims;
- exact proof states only the compiled predicate and frozen scope, never arbitrary semantic absence.

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
