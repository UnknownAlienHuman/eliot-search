# ELIOT Search

**Local-first data preparation and retrieval provider for ELIOT Memory OS.**

> **Status: Architecture 8.4 published; exact P00 contract pack and 45-package swarm scaffold; bounded
> W1–W10 implementation and qualification packets; no business implementation.** Runtime correctness,
> Windows security, provider qualification, performance, Product Pulse and product acceptance remain
> unproven.

## Product boundary

ELIOT Search owns local source observation, immutable revision readback, preparation, rebuildable
retrieval projections, exact scans, compact candidate results, currentness, handles, purge and rebuild.
It is not a memory system, online research service, task controller, canonical knowledge store or client
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

The machine authority is [`swarm/crates.toml`](swarm/crates.toml). Current implementation authorization
is only [`swarm/launch-state.toml`](swarm/launch-state.toml). Future wave packets are bounded read sets,
not permission to start work.

## Prepared product slices

- **W1–W2 / P01–P04:** process/control shell, secrets, provider framing, source admission/identity,
  stable no-execute reads, immutable revision CAS, materialization and unitization.
- **W3 / P05–P07:** lexical profiles, qualified Qdrant process/data plane, exact projection manifests,
  serialized publication, route/epoch pins and exact ordinary reclaim.
- **W4 / P08:** pre-candidate access, bounded planning/execution, exact source-backed validation,
  handles, compact results, continuations, protocol and evaluation seams.
- **W5 / P09–P10:** observation-gap reconciliation, truthful current-workspace preflight,
  saved/unsaved overlays and qualified no-execute Rust tolerant syntax.
- **W6 / P11–P12:** ambiguity-preserving resolution, descriptive lineage/configuration-aware comparison
  and frozen-denominator exact proof.
- **W7 / P13:** restrictive-security linearization, durable handles, CAS mark/sweep, purge/tombstones,
  restore quarantine and lifecycle receipt separation.
- **W8 / P14:** mutually authenticated generic local client edge, standalone CLI and disabled optional
  ELIOT/Research leaf profiles.
- **W9 / P15:** Windows Product Pulse contract with 49 mandatory control cases, 33 metrics and 60
  mandatory G5 probes. Corpus, baselines, environment and acceptance policy remain unselected.
- **W10 / P16–P18:** candidate-specific optional model, document and advanced-scale contracts with
  exact artifact/profile qualification, measured incremental benefit, migration/rollback and complete
  removal back to the accepted P15 baseline.

Handoff indexes are under [`docs/handoff/`](docs/handoff/README.md). External/provider qualification
registries are under [`qualification/`](qualification/). Empty, disabled, unselected or unavailable
records are explicit non-acceptance states.

## Configuration

`search-config` owns only deterministic parsing/layering, provenance, redaction, fingerprints, diffs
and composite reconfiguration planning. Capability packages own their typed sections and runtime
application. Plaintext secrets are invalid; only opaque secret references may appear in configuration.

The example file is safe DIRECT mode. Indexed settings remain disabled or `UNQUALIFIED`. Optional
semantic, document and advanced-scale flags remain false and profile references absent.

## Launch gate

Current P00/W0 order remains:

```text
1. search-contracts
2. after accepted contracts handoff/API digest:
   - search-domain
   - search-ports
3. integration owner publishes W0 receipt
```

Every W1+ package remains blocked. W10 additionally requires:

```text
accepted P15 Product Pulse + independent review
+ one dedicated candidate ADR
+ exact Windows artifact/profile/license qualification
+ measured material incremental benefit
+ complete removal/fallback proof
+ migration/rollback proof when applicable
```

Package presence, Cargo feature, configuration, worker readiness, a model/version name or a successful
unit test cannot authorize optional depth.

## Non-overclaim rules

- retrieval and optional models nominate candidates; exact source readback is still required;
- model rerank cannot add candidates, widen scope, claim completeness or emit client dispositions;
- document workers cannot execute scripts/macros/hooks/shell, use network or follow remote resources;
- exact negative proof uses an authoritative frozen denominator, never Qdrant/top-k/model candidates;
- material ambiguity and incomplete coverage remain explicit;
- Qdrant aliases and worker readiness are not visibility or capability commits;
- optional provider failure cannot break the accepted DIRECT/LEXICAL/CODE baseline;
- active schema/topology is never reinterpreted in place;
- removal restores the accepted P15 handler/profile/route/config before optional physical reclaim;
- secure erase is never claimed without evidence.

## Port rule

Capability/orchestration packages consume `search-ports`. Concrete redb, OS-secret, Qdrant and optional
worker implementations are constructed only by `eliot-searchd`. Vendor/native types, credentials, raw
collections, point IDs and reusable authorization decisions never cross public ports.

## Honest status

`Cargo.toml` files and placeholder `src/lib.rs` / `src/main.rs` establish package boundaries only. P00
still must implement contracts/domain/ports, pin the real Windows-compatible toolchain/dependencies,
generate `Cargo.lock` and execute the real policy/test suite.

```text
business/runtime implementation: absent
accepted package wave receipts: absent
Qdrant/model/document/scale artifacts: unselected
Product Pulse: not accepted
optional depth: disabled and unauthorized
current launch authority: P00 / W0
```

## License

MIT. See [LICENSE](LICENSE).
