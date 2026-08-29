# ELIOT Search

**Local-first data preparation and retrieval provider for ELIOT Memory OS.**

> **Status: Architecture 8.4 published; audited swarm-ready package scaffold; no business implementation.**
> The authoritative standalone contract is
> [ELIOT Search 8.4](docs/architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md).
> The workspace contains package boundaries, dependency declarations and per-package agent contracts
> only. Runtime, correctness, performance, security execution, migration and product acceptance remain
> unproven.

## Product boundary

ELIOT Search owns local source observation, immutable revision readback, preparation, rebuildable
retrieval projections, exact scans, compact candidate results, currentness, purge and rebuild.
It is not the memory system, an online research service, a task controller, a canonical knowledge store
or an authority service.

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
stale or inaccessible candidates are removed before projection.
```

## Swarm decomposition

Architecture S31 permits a capability cell to become one or more packages when a real dependency,
replacement, test, security, runtime or agent-context boundary exists. ADR
[0001](docs/adr/0001-capability-cell-crate-decomposition.md) establishes the one-agent/one-crate
model. ADR [0002](docs/adr/0002-runtime-security-boundary-refinement.md) closes the remaining unowned
or conflated runtime/security boundaries found by the
[pre-implementation audit](docs/handoff/PRE_IMPLEMENTATION_AUDIT.md).

The workspace has **39 library packages** and **4 binary packages**. Family directories are
organizational containers, not forwarding crates.

```text
crates/
  search-contracts/                  C00
  search-domain/                     shared pure invariant kernel
  search-control-redb/               C02
  search-runtime/
    search-runtime-owner/            C01
    search-os-secrets/               OS-bound secret storage support
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
    search-qdrant-supervisor/        qualified process owner
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
    search-handles/                  source/result handle owner
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

Every package contains a small `AGENTS.md` with exact ownership, forbidden responsibilities, logical
API, reason codes, test seams, allowed dependencies and delivery wave. Swarm orchestration uses
[`swarm/crates.toml`](swarm/crates.toml); ordinary agents do not repeatedly load the architecture master.

## Dependency rule

Core capability packages consume vendor-neutral ports. Concrete redb, Qdrant process and Qdrant
transport implementations are composed only by `eliot-searchd`; query packages do not depend directly
on the Qdrant adapter, and lifecycle packages do not open redb or CAS themselves.

## Delivery order

```text
W0 contracts and pure domain
W1 runtime owner, OS secrets, control journal and transport shell
W2 source admission, identity, revision and materialization spine
W3 qualified Qdrant process/data plane, lexical projection, publication and reclamation
W4 lexical query pipeline, handles and compact cards
W5 reconciliation, overlays and Rust structure
W6 subject comparison and exact proof
W7 security, retention, purge and restore hardening
W8 generic client protocol and optional leaf adapters
W9 Product Pulse / Windows qualification
W10 optional model or document depth after accepted P15
```

See [implementation waves](docs/handoff/IMPLEMENTATION_WAVES.md) and the
[crate matrix](docs/handoff/CRATE_MATRIX.md).

## Scaffold semantics

`Cargo.toml` files and `src/lib.rs` / `src/main.rs` files only establish package boundaries. They contain
no retrieval, storage, protocol or runtime implementation. P00 still owns the exact Rust toolchain pin,
Cargo.lock, contract types, tests and dependency-policy proof.

## License

MIT. See [LICENSE](LICENSE).
