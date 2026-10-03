# ELIOT Search

ELIOT Search is a **standalone Qdrant-backed analysis and retrieval framework for AI agents and human
operators**. It is designed to turn very large repositories, portfolios of repositories, research
collections and mixed text corpora into a fast, grounded navigation surface.

The product is not another model loop and not a second search database. Qdrant provides the indexed
retrieval substrate; ELIOT Search supplies the product semantics around it: source acquisition, durable
identity, preparation, projections, query planning, exact verification, provenance, currentness and
compact agent-oriented results.

ELIOT Search runs independently through `eliot-searchd` and `eliot-search`. Integration with ELIOT
Memory OS is optional and uses typed provider contracts over the same standalone owners.

## Product mission

An agent should be able to enter an unfamiliar large corpus and quickly answer:

- what repositories, documents, modules, files, sections and symbols exist;
- where the likely implementation, evidence, tests, documentation and configuration live;
- which exact files or source ranges should be read next;
- which `grep`, regex, symbol or structural checks should verify a hypothesis;
- how implementations differ across repositories and lineages;
- whether a claim is grounded, partial, stale, ambiguous or not exactly proven.

The intended analysis loop is:

```text
orient the scope
→ retrieve likely evidence
→ inspect entities and relations
→ compare implementations or documents
→ verify important claims with the exact plane
→ expand source-backed handles only where needed
```

Heuristic ranking is candidate generation, not truth. Every emitted citation/result is checked against an
exact retained source revision, and complete negative claims require an explicit exact denominator.

See [ADR 0006](docs/adr/0006-agent-analysis-framework-product-scope.md) and the
[agent-analysis product guide](docs/product/AGENT_ANALYSIS_FRAMEWORK.md).

## Qdrant and Search responsibilities

### Qdrant provides

- sparse and optional dense/multivector storage;
- filtered query execution and exact point readback;
- payload indexes and strict-mode enforcement;
- collection-local scoring, filtered IDF and bounded count/scroll operations;
- the physical indexed projection used for fast candidate retrieval.

### ELIOT Search adds

- safe source registration, immutable revisions and no-execute acquisition;
- text/code/document materialization, unitization and coordinate maps;
- deterministic lexical encoding and structural enrichment;
- projection manifests, collision-safe identities, publication epochs and rebuild;
- coherent workspaces, repository lineages and explicit multi-repository portfolios;
- exact, keyword, structural, sparse lexical and optional semantic retrieval planning;
- access/currentness filtering before retrieval and IDF;
- deterministic fusion, diversity, lineage collapse and evidence-role balancing;
- exact source readback, candidate validation, provenance and coverage;
- exact grep/regex/symbol/structural verification;
- progressive compact cards, handles and continuations for agent consumption.

Qdrant payload is never authoritative source evidence, and redb is never a searchable corpus. There is no
Tantivy/Lucene/SQLite-FTS/local-postings fallback in the production design.

## Core workloads

- **Repository orientation:** map an unfamiliar monorepo and identify likely entry points.
- **Precise navigation:** locate symbols, paths, phrases, definitions, callers, tests and documentation.
- **Cross-repository analysis:** compare implementations across an immutable reference portfolio while
  collapsing forks, mirrors and copies.
- **Research/document analysis:** navigate long text collections with source-backed sections, references,
  tables and coordinates as qualified materializers become available.
- **Current workspace search:** combine published Qdrant projections with saved/unsaved overlays without
  silently serving stale results.
- **Exact verification:** compile and execute complete grep/regex/symbol/structural scans independently of
  top-k retrieval.
- **Corpus intelligence:** profile and diff corpora, trace provenance and expose truthful coverage gaps.

The current eleven v1 recipes cover exact navigation, entity inspection, comparison, corpus profile/delta,
provenance and exact scans. The missing first-class free-text retrieval and scope-orientation contracts
are tracked in [#213](https://github.com/UnknownAlienHuman/eliot-search/issues/213).

## Product boundary

ELIOT Search owns:

- source admission, identity, revision retention and preparation;
- lexical, structural and optional semantic projection construction;
- Qdrant supervision, schema, publication and retrieval;
- access-scoped query planning, candidate validation and result projection;
- currentness, handles, rebuild, retention and recovery;
- standalone daemon/CLI and optional provider adapters.

It does **not** own Tasks, WorkScopes, GM/Governor state, agent scheduling, native harness lifecycle,
assignment tickets, writer leases, mailboxes, review acceptance or finish authority. Those responsibilities
belong to [`eliot-swarm-controller`](https://github.com/UnknownAlienHuman/eliot-swarm-controller) and,
after integration, [`eliot-memory-os`](https://github.com/UnknownAlienHuman/eliot-memory-os).

The repository's remaining `swarm/**` files are advisory/historical development metadata. They are not
runtime inputs or prerequisites for implementing or running Search. Residual controller tooling is tracked
for removal in [#214](https://github.com/UnknownAlienHuman/eliot-search/issues/214).

## Architecture baseline

- Rust 1.98 workspace;
- Qdrant is the only indexed/search database;
- redb stores bounded technical control state;
- immutable source/preparation evidence remains outside Qdrant;
- one source view and one coherent access/currentness snapshot bind each query;
- Qdrant nominations require exact source/revision validation before emission;
- standalone DIRECT remains independent of indexed-mode availability;
- ELIOT integration is a leaf adapter, not a reverse authority path.

Read the [architecture entry point](docs/architecture/README.md). Architecture Part I is the normative
product contract. The embedded Part II Codex handoff is historical scaffolding and does not reinstate a
P00/ticket/lease implementation gate.

## Current status

Implementation is incomplete and this is not a qualified production release.

The active completion spine is:

```text
source admission / immutable revisions / preparation
→ typed lexical/structural projections
→ real Qdrant publication
→ access-scoped retrieval and filtered IDF
→ authoritative candidate readback and validation
→ bounded standalone/provider results
```

Indexed readiness remains disabled until the exact Qdrant artifact, bridge, schema, query, restart and
end-to-end qualification gates pass. Planning records, source presence and mock/oracle execution are not
qualification.

Important open product work includes:

- agent free-text retrieval and scope orientation: #213;
- cleanup of residual controller tooling: #214;
- large multi-repository scale qualification: #215;
- research/document materialization and navigation: #216;
- the existing Qdrant identity/projection/publication/daemon integration stack: #200, #207, #209, #210,
  #211.

## Workspace

Key entrypoints:

- `bins/eliot-searchd` — standalone daemon and composition root;
- `bins/eliot-search` — local client/CLI;
- `crates/search-index-qdrant/search-qdrant-bridge` — sole Qdrant vendor adapter;
- `crates/search-index-qdrant/search-publication` — epoch-safe publication coordinator;
- `crates/search-query/*` — access, planning, retrieval, exact verification, validation and projection;
- `crates/search-lexical` — deterministic document/query sparse encoding;
- `crates/search-source/*` — admission, identity, revisions, materialization and unitization;
- `crates/search-control-redb` — bounded technical control state.

## Build

Use the pinned toolchain and locked dependencies:

```powershell
cargo +1.98.0 check --workspace --all-targets --all-features --locked
cargo +1.98.0 clippy --workspace --all-targets --all-features --locked -- -D warnings
```

During implementation, run the smallest applicable package gate first:

```powershell
cargo +1.98.0 check --locked -p <package> --all-features
cargo +1.98.0 clippy --locked -p <package> --all-features -- -D warnings
```

Full product/process tests are performed after the implementation spine is complete, with focused tests
added earlier where a change needs immediate causal proof.

## Running

Standalone is the supported baseline. Exact startup syntax is still evolving with provider transport,
redb cutover and Qdrant supervisor integration; do not infer readiness from legacy experimental commands.

The final baseline must support installation without ELIOT Memory OS, source/corpus registration, durable
restart-safe state, real Qdrant indexing/search, index rebuild, local CLI use and optional ELIOT provider
integration without a second state owner.

## Repository workflow

Read `AGENTS.md`, Architecture Part I, the nearest package instructions and the named issue/PR before
editing. Use one branch/worktree per manager and avoid overlapping package writers.

No ticket/lease/signature issuance protocol is required. Do not implement generic agent-controller
features here. Keep GitHub Actions manual-only and never claim unexecuted gates.

## License

See [`LICENSE`](LICENSE).
