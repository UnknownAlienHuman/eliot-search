# Documentation map

| Directory | Contents |
|---|---|
| `architecture/` | Architecture entry point and normative Architecture 8.4 Part I. |
| `product/` | Product mission and agent-analysis behavior derived from the architecture. |
| `adr/` | Accepted product, implementation and boundary decisions. |
| `contracts/p00/` | Bounded field-level contract projection, recipes, reasons and ports. |
| `config/` | Product configuration contracts and section ownership. |
| `current/` | Current-workspace, observation and overlay contracts. |
| `client/` | Standalone client and optional provider-edge contracts. |
| `evaluation/` | Product, quality, scale and release evaluation contracts. |
| `optional/` | Disabled-by-default optional depth profiles. |
| `handoff/` | Historical/package planning notes; not implementation authority. |
| `audit/` | Dated observations; they accept and authorize nothing. |
| `execution/` | Historical run records and diagnostics; not product architecture. |
| `generated/` | Generated product schemas/descriptors after their owning contracts are accepted. |

## Product entry points

Read in this order:

1. [`architecture/README.md`](architecture/README.md) — how to read the combined master;
2. Architecture Part I in `architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md`;
3. [`adr/0005-standalone-search-product-and-controller-boundary.md`](adr/0005-standalone-search-product-and-controller-boundary.md);
4. [`adr/0006-agent-analysis-framework-product-scope.md`](adr/0006-agent-analysis-framework-product-scope.md);
5. [`product/AGENT_ANALYSIS_FRAMEWORK.md`](product/AGENT_ANALYSIS_FRAMEWORK.md);
6. the nearest accepted contract/package instructions and current product issue/PR.

Architecture Part I defines the source/Qdrant/query/evidence system. ADR 0005 defines the external
controller boundary. ADR 0006 clarifies that the product is an agent-oriented large-corpus analysis
framework, not merely a path/symbol locator.

## Authority

Use this order:

1. Architecture **Part I**;
2. accepted product ADRs;
3. accepted public product contracts;
4. nearest package instructions and current issue/PR;
5. compiled source/dependency reality;
6. historical planning material.

Part II (`Codex Handoff 2.7`) inside the master is historical scaffolding. Its P00 sequencing and old
repository-control assumptions do not authorize or block current product work.

`swarm/**`, `docs/handoff/**` and `docs/execution/**` do not authorize or block implementation. In
particular, `swarm/launch-state.toml`, ticket drafts and lease records are legacy/advisory coordination
metadata, not a current permission system.

## Standalone boundary

ELIOT Search is independently installable and runnable. ELIOT Memory OS and ELIOT Swarm Controller are
external consumers/controllers, not Search runtime dependencies. Search integration with ELIOT remains a
typed leaf adapter over the same standalone state and query owners.

## Analysis boundary

Qdrant is the sole indexed retrieval backend. Search adds safe source preparation, lexical/structural
projections, explicit multi-repository portfolios, query planning, deterministic fusion, source-backed
validation, exact proof, provenance, coverage and compact navigation for agents.

Heuristic retrieval identifies likely evidence and where to inspect or grep next. It never replaces exact
source readback or complete-denominator verification.

## Qualification

External-artifact and product-evidence inputs live under repository-root `qualification/`. A packet or
captured diagnostic is not a passing receipt. Capabilities remain disabled until the exact accepted probes
run at the exact product revision and receive independent review.
