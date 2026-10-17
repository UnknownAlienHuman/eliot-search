# ELIOT Search: remaining product outcomes

**Source checkpoint:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`, reviewed 2026-10-17.
This map preserves obligations, not a ready-task queue or product PASS. Current maintainer handoff,
[#352](https://github.com/UnknownAlienHuman/eliot-search/issues/352), the actual active issue/PR and
source determine the next slice under [EXECUTION_PROTOCOL](EXECUTION_PROTOCOL.md).
Preserve [#598](https://github.com/UnknownAlienHuman/eliot-search/issues/598) and its manager/WIP.

Part I and accepted ADRs define semantics. A required predecessor means its concrete API/artifact,
not closure of its whole programme. Source-complete, integrated and qualified are different outcomes.
The [previous detailed map](https://github.com/UnknownAlienHuman/eliot-search/blob/927265d1dd71f3d355131f38ac73365e6558a77e/docs/product/PROJECT_COMPLETION.md)
preserves historical issue transfers and phase numbering; its old queue and status are superseded here.

## 1. Reuse delivered source

| Existing result | Remaining boundary |
|---|---|
| #237/#319 canonical hashing; #258/#323 indexed contracts | Reuse existing `search-contracts` APIs. Remaining producer migrations do not justify another codec/hash owner. |
| #256/#338 S11; #257/#342 complete UnitSet | Preserve their implementations; retire actual root legacy identity callers under #329. Preparation/persistence is a separate integration boundary. |
| #259/#519 planner and lookup repair | Complete real input/profile/lineage integration; do not port the same algorithm again. |
| #260 / PR #605 submit/restore canonical admission | Active empty UnitSet remains unsupported. Actual journal/effects/readback remain #261/#263. |
| Reusable provider client | [`search-provider-client`](../../crates/search-provider-client/) exists; [CLI imports](../../bins/eliot-search/src/provider_client.rs) already use it. Residual old line/TCP facade and native attachment are #332, not a reason to recreate the client. |
| #253 / PR #322 Unicode fold decision | Production prose folding remains #254. The decision is no longer pending. |
| Existing lexical/document library adapters; PR #608 operation budgets | Consume their exact profiles and real preparation callers. Source presence is not corpus/daemon qualification. |
| PR #609 static Qdrant artifact admission | Exact live/native server/client/schema/IDF qualification remains distinct. |

The [verified audit](../audit/ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md) contains source anchors and
corrections. Dated F01–F154 findings in the [master audit](../audit/ELIOT_SEARCH_MASTER_AUDIT_2026-10-09.md)
and specialized audits retain their obligations until an actual source result or explicit disposition
resolves them; a closed planning issue alone does not.

## 2. Complete the existing owners

Rows are outcome groups, not single PRs or all-or-nothing predecessor gates. Select one missing
state/effect transition with its actual inputs, immediate consumers and compatibility boundary.

| Boundary / existing work | Required result and consolidation |
|---|---|
| Root/control/config: #266/#267/#268/#269/#238/#333 | One typed native installation/root authority; inspect creates nothing. Apply durable config and one guarded control state. Preserve actual admitted handles/security observations; retire each old writer only at its real cutover. |
| Secrets and encrypted storage: #307–#310/#330/#598 | Purpose/selection/candidate/retiring keys, finite leases, native effects/readback and key-bound AEAD. Continue admitted held body → protected immutable object → durable occurrence/head/residency/retention link → exact reopen. No second daemon selector or caller-shaped receipt. |
| Preparation/ingestion/currentness: #331/#270/#271/#272/#239/#241 | Materialization/maps/UnitSet persist as one verified bundle; pre-admission precedes bytes. Revalidate all roots for a coherent/partial SourceView and explicit immutable corpus/portfolio population. Watcher quietness is not currentness. |
| Git source: #129/#335 | Bounded, no-execute local object acquisition through a pinned read-only adapter over admitted storage. Current loose-object validation is not a pack/ref backend. Complete missing behavior without importing a whole Git runtime. |
| Publication/vendor: #261/#262/#263/#329/#120/#310 | Consume shared S11/payload/eligibility contracts; delete bridge copies. Durable intent → exact Qdrant effect/readback → guarded redb VisibleEpoch/route; alias movement is not the commit point. Reconcile unknown outcomes, never blindly replay. |
| Access/bindings: #194/#287/#288/#274 | One durable binding/grant snapshot and owner-issued scoped plan; live restrictive revalidation. Same eligibility governs retrieval, IDF, denominator, statistics and validation; no post-filter-only access. |
| Exact ranges/paging: #246–#249/#275/#276/#282–#285/#300 | Existing matching engines plus exact frozen denominator. Handles reauthorize retained ranges; continuations own finite pins, expiry and atomic emission/terminal cleanup. Missing/denied/gap is not absence. |
| Query/ranking/result: #278/#290/#291/#293/#294/#279 | One finite work budget, actual retrieval and retained-source validation; deterministic exact arithmetic/ties, explicit leg outcomes, truthful coverage and bounded cards. Remove parallel handler trees when real consumers cut over. |
| Public client/recipes: #235/#332/#277/#280/#281/#213 | One native endpoint, parser, serializer and reusable session owner. Serve the eleven stable v1 recipes and separately versioned `retrieve_evidence@1`/`orient_scope@1`. Runtime capability state derives from actual owners, never configuration or a badge. |
| Analysis: #296–#298/#232/#223/#224 | Source-backed resolution, ambiguity, comparison, independent lineage and recommended reading. Reuse Tree-sitter syntax/SCIP facts where qualified; heuristic orientation never becomes exact evidence or a task verdict. |
| Lexical/document profiles: #252/#254/#226/#227/#216 | Exact pinned Unicode and raw-coordinate mappings; query/document parity. Existing Markdown/XML event adapters feed shared preparation. Isolated workers provide separately qualified document profiles, not a universal hostile in-process parser. |
| External observations: #228–#231 | Bounded connector checkpoints, gaps, tombstones, retries and immutable raw observations committed with retained identity. Remote title, latest alias or citation edge is not source truth. |
| Lifecycle: #301–#305/#243–#245/#134 | Restrict visibility before reclaim. Complete bounded reachability across live data, pins, leases, operations, backups, restores, rollback copies and key generations. Same-key restore and new-key migration are separate recoverable transitions; retain last copies/keys until exact owner release. |
| Tooling/deletion: #189/#214/#315/#316/#329/#525 | Delete obsolete implementations and repeated metadata after consumer census. Preserve real product guards and byte profiles; do not replace a tooling catalogue with a new controller framework. |
| Evaluation/release: #233/#234/#240/#242/#264/#119/#215/#137/#140 | Frozen installed candidate, exact integration and qualification evidence, independent review and explicit human publication of the same bytes. Tooling/verifier source may precede product integration; their results cannot be fabricated in advance. |

Shared APIs precede their consumers; real integration precedes qualification. Configured means parsed
and accepted, operational means actual owners can perform the operation, release-qualified means exact
candidate evidence passed. Startup must not require its own future evaluation report. DIRECT remains
usable under Qdrant failure. Normal startup/ingestion/recovery never requires Memory OS or a controller.

## 3. Reduce before adding another mechanism

Use the [measured reduction map](../audit/ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md#7-measured-reduction-work)
and [reuse index](../audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) to choose an actual deletion or
replacement. Start with obsolete daemon harness owners, copied identity/schema/hash code and generated
repository metadata. No arbitrary package-count target or cosmetic split qualifies as progress.

A selected reduction slice records the old entry/target, callers, replacement owner, byte disposition,
minimal retained regression cases and net added/deleted code by category. New integration may add code;
state what it enables and which old path it removes. Do not maintain both implementations indefinitely.
Do not promise a sub-100,000-line product before production/test/legacy ownership is measured correctly.

## 4. Preserve optional scope explicitly

[ADR 0006](../adr/0006-agent-analysis-framework-product-scope.md) remains the product mission. Optional
work is not erased by shortening this map, and no optional profile becomes a baseline dependency by
being mentioned here. #139 records each as implemented, disabled or not shipped; no unresolved status
counts as complete. Each shipped profile adds its own exact artifact/data/features and acceptance.

| Profile group | Existing owners and intended mechanism |
|---|---|
| Code facts | #223 Tree-sitter syntax; #224 SCIP precise imports; #225 Ctags heuristic symbols; #236 one SARIF importer for qualified finding providers. No second graph/index authority. |
| Documents | #226 Markdown/JATS; #216 HTML/Tika, inert LaTeX syntax, named PDF parser, scholarly GROBID, Office/Docling, explicit legacy parser allowlist, OCRmyPDF/Tesseract and pinned language packs. #227 owns isolated worker bounds. No macro/resource/TeX execution or silent OCR. Exact source/loss/native-coordinate maps remain required. |
| Research corpora | #228 connector core; #229 OpenAlex snapshots; #230 Semantic Scholar releases; #231 bounded Crossref/OpenCitations observations. Preserve contradictions, exact record identity and corpus versions. |
| Client depth | #130 memory-only LSP/overlay with TTL/snapshot/reauthorization; #219 dated tools-only MCP leaf over the shared client. Neither owns another planner/database or inherits core acceptance. |
| Semantic depth | Optional embeddings/rerank under [accepted profiles](../optional/README.md); model artifacts and limits are separately qualified. Deterministic lexical/code baseline requires no LLM. |

Directions are not floating dependency approvals. Exact donor choices and rejected alternatives remain
in existing issues/specialized records; use the accepted pin when still valid and verify a new adoption
once. Do not reopen completed donor research or introduce every optional framework up front.

## 5. Qualification and release

After complete integration, #264 exercises a fresh generation through retained source, preparation,
publication, Qdrant readback, access-scoped retrieval/IDF, exact source validation, bounded public output
and restart/unknown-outcome recovery. #119 qualifies the exact server/client/config/schema/native
binding. Build features, static artifact admission and successful compilation do not establish either.

#215 freezes candidate bytes, sources/artifacts/config, population, profiles, cases, resources and SLOs
before measured results. Baseline lanes remain M1 installed product, W1 root/watcher/currentness,
P1 deterministic retrieval, P2 complete exact proof, I1 publication/lifecycle, R1 total resources and
N1 10,000-query no-write/disclosure. Every shipped document/code/research/adapter tier adds D1/C1/S1/A1.
Preserve FAIL/PARTIAL/UNAVAILABLE/NOT_RUN in the denominator and retain raw evidence/identities/counters.
Signatures, histograms and author-written approved booleans do not replace exact subjects.

Release requires current migration/removal dispositions, no active imports of replaced legacy APIs,
reviewed normative version/hash and operator guide, reproducible locked/offline inputs, complete SBOM/
licenses/non-Cargo artifacts and standard-user Windows install/provision/restart/upgrade/uninstall.
Include backup/last-copy/key protections. Independent reviewers reopen/recompute exact evidence. Fixes
create a new candidate/run; never alter expectations or rebuild/sign different bytes after accepted runs.
Publication requires explicit human authorization for the exact tested signed subject.

## Maintenance

Update affected rows after source merges; use the current issue body for slice details and scheduling.
Do not append another launch packet, duplicate execution rules, impose a subagent quota or require a
new full-repository audit before every task. Historical evidence remains in Git and specialized audits.
This document does not assert compilation, native execution, live-Qdrant results or product acceptance.
