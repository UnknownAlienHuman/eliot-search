# Workspace member disposition

Workspace source: `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`. Recounted 2026-10-21.
This is a proposed receiving-owner/removal map, not a package-count target or blanket rewrite instruction.
KEEP means retain the useful responsibility and correct existing code; rewrite only defective mechanisms.
MERGE means move useful behavior to its consumer. DELETE removes the current package after callers move.
OPTIONAL requires a selected consumer, not a scaffold. CONDITIONAL concerns the #598 object/key handoff only.

**52 baseline rows: 17 KEEP, 11 MERGE, 16 DELETE, 6 OPTIONAL, 2 CONDITIONAL.** The previous 17/15/13/5/2
summary and 'all other 48' claim were arithmetic errors. With six optional and two conditional rows, 44 had
unconditional baseline dispositions. None of these counts is a desired final workspace size.

R030 removes the optional `search-eval` scaffold and its Cargo/lock wiring after confirming that it had no
external Rust caller. The current workspace therefore has 49 explicit members. The historical row remains below
to preserve the baseline disposition and receiving issue; it is not an instruction to recreate the crate.

## Contracts, control and runtime

| Member | Disposition | Receiving responsibility / issue |
|---|---|---|
| `search-contracts` | KEEP | Small actual shared IDs, locations and current DTOs; #280/#259. Remove planned proof families. |
| `search-domain` | MERGE | Rules belong beside the source/publication/query operation that uses them. |
| `search-ports` | DELETE | Concrete owners; narrow consumer-local trait only where actually useful, not a new global catalogue. |
| `search-config` | KEEP | Typed settings and bounded decode; #333. May inline if small. |
| `search-control-redb` | KEEP | Typed current rows and transactions; #270/#261/#333. |
| `search-runtime-owner` | KEEP | Real root/lock lifetime; #266/#360. |
| `search-os-secrets` | DELETE | Selected credentials belong to concrete storage/Qdrant/client owners; coordinate #598 before moving its consumers. |
| `search-revision-crypto` | CONDITIONAL | Accepted storage primitive only; #598/#330. |
| `search-os-secrets-windows` | CONDITIONAL | Actual Windows key effects for the selected storage profile; #598/#330. |
| `search-provider-transport-windows` | DELETE | Old paired-client adapter; #332. Future persistent transport is not required by child stdio. |
| `search-retention` | DELETE | Current/pending/live reference GC and publication cleanup; #302/#303. |

## Source and preparation

| Member | Disposition | Receiving responsibility / issue |
|---|---|---|
| `search-source-admission` | MERGE | Small pre/post-read policy in ingestion; #270. |
| `search-source-admission-receipt` | DELETE | No permanent admission archive; #270. |
| `search-source-provenance` | MERGE | Actual source kind/locator/profile metadata; #270/#331. |
| `search-source-records` | MERGE | Current typed root/source/revision rows; #270. |
| `search-source-history` | MERGE | Needed head/occurrence semantics only; #270/#259. Respect #598 overlap. |
| `search-source-registry` | MERGE | Current bounded source lookup, not registry replay; #270/#271/#272. |
| `search-source-identity` | MERGE | Actual logical source identity and Git object algorithm; #270/#335. |
| `search-source-reconcile` | MERGE | Dirty queue/inventory/conservative retirement; #271. |
| `search-safe-reader` | KEEP | Stable selected-root/Git read; #267/#335. |
| `search-revision-store` | KEEP | One immutable store; #330 and coordinated #598 handoff. |
| `search-materializer` | KEEP | Retained bytes to mapped text/structure; #331. |
| `search-unitizer` | KEEP | Structure-first units and coordinates; #331/#259. |
| `search-code-enricher` | OPTIONAL | Actual outline producer for navigation; #281. Remove current heuristic scaffold if unselected. |
| `search-lexical` | MERGE | Useful bounded identifier/context text expansion; #331/#263. No second BM25 engine. |

## Index and publication

| Member | Disposition | Receiving responsibility / issue |
|---|---|---|
| `search-projection-planner` | KEEP | Bounded actual mutation/manifest pages; #259. No whole-plan proof graph. |
| `search-point-identity` | KEEP | Stable retrieval identity and collision check; #329/#259. Replace revision-bound S11 v1. |
| `search-qdrant-supervisor` | DELETE | Configured endpoint baseline; managed child only if separately selected (#334). |
| `search-qdrant-bridge` | KEEP | One concrete SDK runtime and operations with current callers; #263. |
| `search-publication` | KEEP | Pending/current/unfinished-cleanup ownership; #261. |
| `search-epoch-pins` | DELETE | Private current-view guards in publication; #300. |
| `search-index-reclaimer` | DELETE | Exact cleanup effect in publication/bridge; #303. |

## Query

| Member | Disposition | Receiving responsibility / issue |
|---|---|---|
| `search-access` | MERGE | Non-widening scope and real optional session intersection; #272/#274. |
| `search-overlay` | OPTIONAL | Selected live editor buffer integration, not current overlay OS; #336. |
| `search-exact` | KEEP | Retained inventory scan and matcher; #249/#248. |
| `search-subject-resolver` | OPTIONAL | Bounded source-backed navigation where real facts exist; #281. |
| `search-query-planner` | DELETE | Direct handlers; route grouping only when needed; #279. |
| `search-retrieval-executor` | DELETE | Request context and backend call in query owner; #291/#278. |
| `search-candidate-validator` | DELETE | Manifest lookup and retained read in evidence owner; #293. |
| `search-comparator` | OPTIONAL | Source-backed comparison, not supplied-digest proof; #281. |
| `search-handles` | DELETE | Item locators in the same live query window; #275. |
| `search-result-projector` | MERGE | Final-order bounded card formatting; #293/#279. |
| `search-continuation` | DELETE | Session-owned window/cursor in query owner; #275. |

## Interface, binaries and tooling

| Member | Disposition | Receiving responsibility / issue |
|---|---|---|
| `search-eval` | OPTIONAL → REMOVED R030 | #233 may later add a small pure post-product evaluator over real result files; no crate/runtime authority is retained. |
| `search-provider-protocol` | KEEP | Current bounded framed exchange; #280. |
| `search-provider-client` | KEEP | Owned local I/O; #280/#332. Public sync API does not imply blocking cancellation. |
| `search-eliot-adapter` | DELETE | Optional later MCP leaf reuses the real current client; #219. |
| `eliot-searchd` | KEEP | One actual application and dispatcher; #264. |
| `eliot-search` | KEEP | One-shot and live-session behavior over the same client; #332. |
| `eliot-search-model-worker` | DELETE | Unavailable stub; no model framework before a selected implementation. |
| `eliot-search-doc-worker` | OPTIONAL | Real PDF path (#227) or remove refusal stub. |
| `xtask` | DELETE | Retired governance; keep only a proven useful small command if any survives; #214/#525. |

Package deletion does not delete product requirements such as navigation, comparison or supported documents.
Use the manifesto and selected feature scope; do not rebuild optional frameworks merely because historical rows
exist. Full local source maps hold defect details; this table does not repeat their algorithms.

Move real callers, useful causal cases and necessary shared data with the responsibility. Remove obsolete
imports, fixtures and instructions together. Existing healthy code may remain, and implementation may merge
more packages where no useful boundary survives. No package row authorizes touching private #598 work.
