# ELIOT Search

**Local-first data preparation and retrieval provider for ELIOT Memory OS.**

> **Status: Architecture 8.4 published; ownership-hardened swarm contracts; no business implementation.**
> The authoritative standalone contract is
> [ELIOT Search 8.4](docs/architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md).
> The workspace contains package boundaries, dependency declarations, bounded assignments, port
> ownership and launch gates. Runtime correctness, performance, security execution, migration and
> product acceptance remain unproven.

## Product boundary

ELIOT Search owns local source observation, immutable revision readback, preparation, rebuildable
retrieval projections, exact scans, compact candidate results, currentness, purge and rebuild.
It is not the memory system, online research service, task controller, canonical knowledge store or an
authority service.

ELIOT Memory OS reaches Search through typed provider contracts. It never shares canonical database
credentials with Search. Search returns candidates, coverage, freshness, assurance and reason codes;
the consuming client owns interpretation and admission.

## Non-negotiable invariants

```text
one search/index database: Qdrant;
redb is a bounded control journal, never a searchable corpus;
source bytes or an immutable admitted revision are source truth;
one point has one ProjectionMembership; no membership arrays;
access/currentness filters run before retrieval and IDF;
indexed top-k never defines an exact negative denominator;
live deny and purge fences override query snapshots immediately;
uncommitted epochs are never current and are never reused;
publication writes are acknowledged and read back before commit;
unsaved editor bytes never persist without explicit admission;
vendor types never cross public package ports;
stale or inaccessible candidates are removed before projection;
handle possession never grants access;
ordinary index reclaim is not security purge.
```

## Swarm decomposition

Architecture S31 permits a capability cell to split when a real dependency, replacement, test,
security, runtime or agent-context boundary exists. ADR
[0001](docs/adr/0001-capability-cell-crate-decomposition.md) established one agent per crate. ADR
[0002](docs/adr/0002-runtime-security-boundary-refinement.md) separates the remaining process,
secret, admission, reclaim and handle-state owners.

The workspace has **39 library packages** and **4 binary packages**. Family directories are navigation
containers, not forwarding crates.

```text
crates/
  search-contracts/                  C00
  search-domain/                     shared pure invariant kernel
  search-control-redb/               C02
  search-runtime/
    search-runtime-owner/            C01
    search-os-secrets/               OS-bound secret support
    search-retention/                C28
  search-source/
    search-source-admission/         admission-policy evaluator
    search-source-registry/          C03
    search-source-identity/          C04
    search-source-reconcile/         C05
    search-safe-reader/              C06
    search-revision-store/           C07
  search-prep/
    search-materializer/             C08
    search-unitizer/                 C09
    search-code-enricher/            C10
  search-lexical/                    C11
  search-model-provider/             C12 optional
  search-index-qdrant/
    search-projection-planner/       C13
    search-point-identity/           C14
    search-qdrant-supervisor/        qualified local process owner
    search-qdrant-bridge/            C15 data plane
    search-publication/              C16
    search-epoch-pins/               C17 pin registry
    search-index-reclaimer/          C17 reclaim executor
  search-query/
    search-access/                   C18
    search-overlay/                  C19
    search-exact/                    C20
    search-subject-resolver/         C21
    search-query-planner/            C22
    search-retrieval-executor/       C23
    search-candidate-validator/      C24
    search-comparator/               C25
    search-handles/                  handle state/expansion owner
    search-result-projector/         C26
    search-continuation/             C27
  search-eval/                       C29
  search-provider-protocol/          C30 generic edge
  search-eliot-adapter/              C30 optional ELIOT profile
  search-research-export-adapter/    C30 optional Research profile

bins/
  eliot-searchd/
  eliot-search/
  eliot-search-model-worker/         optional after P15
  eliot-search-doc-worker/           optional after P15
```

## Agent context

Each writer receives one bounded packet under
[`swarm/assignments/`](swarm/assignments/README.md), package/family/root `AGENTS.md`, the assignment
protocol and accepted dependency handoffs. Ordinary package agents do not load the 145 KB architecture
master.

Current implementation permission comes only from
[`swarm/launch-state.toml`](swarm/launch-state.toml). A Cargo member or future wave is not launch
authority.

## Port and adapter rule

Capability/orchestration packages consume the vendor-neutral ports in
[`PORT_CATALOG.md`](docs/handoff/PORT_CATALOG.md). Concrete redb, OS-secret, Qdrant process and Qdrant
data-plane adapters are constructed only by `eliot-searchd` and never travel through public query or
lifecycle APIs.

## Delivery order

```text
W0 contracts and pure domain
W1 runtime owner, OS secrets and bounded control shell
W2 admission, source/revision/materialization spine
W3 qualified Qdrant process/data plane, lexical projection, publication and reclaim
W4 lexical query pipeline, handles and compact cards
W5 reconciliation, overlays and Rust structure
W6 subject comparison and exact proof
W7 security, retention, purge and restore hardening
W8 generic client protocol and optional leaf adapters
W9 Product Pulse / Windows qualification
W10 optional model or document depth after accepted P15
```

See [implementation waves](docs/handoff/IMPLEMENTATION_WAVES.md), [crate matrix](docs/handoff/CRATE_MATRIX.md),
[dependency graph](docs/handoff/DEPENDENCY_GRAPH.md), [port catalog](docs/handoff/PORT_CATALOG.md) and
[P00 bootstrap](docs/handoff/P00_BOOTSTRAP.md).

## Scaffold semantics

`Cargo.toml` and empty `src/lib.rs` / `src/main.rs` files establish package boundaries only. No
retrieval, storage, protocol or runtime behavior has been implemented. P00 still owns the exact Rust
toolchain pin, Cargo.lock, contract implementation, pure tests and dependency-policy proof.

## License

MIT. See [LICENSE](LICENSE).
