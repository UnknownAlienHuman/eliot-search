# Analysis registry

Continuation memory for the current refactor. It records inspected evidence, decisions, published changes and
unrun work. It is not architecture, a permission system or a reason to delay known fixes.

## Resume after interruption

**Checkpoint:** R042, 2026-10-24 — package-currentness and maintenance ownership reconciled.  
**Branch:** `codex/refactor-preparation-20261017`, [PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610).  
**Inspected head:** `6de4d2b3788b94714b59351cdb5dc5d0f0d13fd5` (R041).  
**Original source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`.

R042 extends the R041 revision-currentness rule to source, runtime, preparation, code-enricher,
Qdrant-supervisor and overlay package instructions. It also corrects local maintenance ownership: stopped-root
object collection/archive/import stays a documentation owner after `search-retention` Cargo removal, while online
Qdrant retirement belongs to publication cleanup.

The persistent source → retained evidence → Qdrant → CLI application remains unconnected. Read
[docs/README](../README.md), then the affected owner recipe and exact current callers. Do not repeat R027–R042
as another repository-wide audit.

**Next implementation:** #333 typed settings and real consumers, then #270/#330/#331/#271
source/object/preparation/reconciliation and #264 the connected product path. Optional document/MCP/regex/
watcher/model/overlay/managed-Qdrant work remains behind its prerequisite and implementation-time donor/caller
selection. Coordinate only actual #598 overlap.

Current workspace has **49 explicit members**. This is inventory, not a target package count.

## Established decisions — do not rediscover

- Documentation authority is architecture → implementation → nearest `FUNCTIONS.md`; product docs own
  workflows, this registry owns evidence/status, and the donor register owns upstream facts.
- One current-format application owns root, source/object, preparation, publication, query and local session.
- Config receipts do not apply values; typed settings must reach actual consumers.
- Query/publication/rebuild composition still contains disconnected oracle/in-memory/supplied-proof paths.
- Historical size and caller measurements are revision-specific evidence, not permanent current facts or
  deletion targets.
- Before deleting or merging a dormant package, recheck exact current manifests, Rust/non-Rust callers, feature
  and target wiring, tests and workflows.
- An inspected donor is not installed. Exact version, features, MSRV, advisories and native artifact move with
  the real consumer implementation.
- Literal Exact is baseline; optional regex cannot block it or reserve a speculative dependency.
- Reconciliation must work independently before optional watcher hints.
- `search-retention` is not a Cargo package to restore. Its remaining documentation owns stopped-root local
  object collection and archive/import; Qdrant cleanup remains publication work.
- `rmcp`, PDFium/`pdfium-render`, `regex-automata`, `notify`, high-level TOML/Serde and model runtimes are absent
  from the current lock unless a later implementation commit explicitly adds them.
- R031 pins `pulldown-cmark 0.13.5`; #441 is closed as that dependency task.
- #598 private object/key work remains protected; consume only its accepted public handoff.

## Delivery remaining

| Group | Current owners/issues | Missing product result |
|---|---|---|
| Invocation/root/settings/source/preparation | [implementation owners](../implementation/README.md#owners), #333/#266/#270/#330/#331/#271 | Validated settings reach owners; retained current head and recoverable reconciliation/preparation. |
| Identity/Qdrant/publication | #259/#329/#263/#261/#300/#303 | Real SDK effects, atomic evidence/view, restartable rebuild and finite cleanup. |
| Query/evidence/window | [query](../../crates/search-query/README.md), #272/#279/#291/#293/#275/#249 | Scoped retrieval and baseline Exact, pinned retained evidence and one bounded live-session window. |
| Protocol/client/CLI | #280/#332/#264 | One serviced framed child session and useful default CLI result. |
| Maintenance/archive/import | #302/#244 | Stopped-root collection, complete archive and fresh-destination import. |
| Product depth | [product behavior](../product/AGENT_ANALYSIS_FRAMEWORK.md), #281/#227/#335 | Source-backed orientation/comparison/documents/Git behavior. |

Optional regex (#248), watcher (#239), evaluation (#233), MCP (#219), managed Qdrant (#334), overlays (#336) and
models follow working prerequisites and do not form baseline subsystems.

## Coverage ledger

| ID | Area | Status | Current result / remaining |
|---|---|---|---|
| A001 | Documentation authority | CHECKED for R032–R042 scope | Canonical indexes, package handoffs, donor state, currentness rules and maintenance ownership are aligned for inspected paths. |
| A002 | Historical governance | CHECKED | Removed as current authority; Git preserves history. |
| A003 | Workflows/wrappers | CHECKED | Actions remain manual-only. |
| A004 | Workspace inventory | CHECKED for inventory | 49 members; current full workspace/binaries remain unrun. |
| A005 | Entrypoints/product | PARTIAL | Shadowing mapped; real application absent. |
| A006 | Root/config/control | PARTIAL | Typed settings/root/current state not implemented. |
| A007 | Sources/reconciliation | PARTIAL | Sync entrypoint unavailable; watcher correctly deferred. |
| A008 | Private object/key work | PROTECTED | Consume accepted #598 handoff only. |
| A009 | Preparation/identity | PARTIAL | Markdown identity updated; connected preparation remains. |
| A010 | Qdrant/donors | CHECKED for named static questions | Live integration/quality/scale unrun. |
| A011 | Publication/view/cleanup | PARTIAL | Real effects/current-view path absent. |
| A012 | Query/evidence/window | PARTIAL | Connected path absent; optional regex/overlay remain uninstalled or dormant. |
| A013 | Protocol/client/CLI | PARTIAL | Serviced session absent. |
| A014 | Product depth | PARTIAL | Requirements preserved; implementations incomplete. |
| A015 | Tests/governance code | PARTIAL | Remaining fixtures need caller cutover. |
| A016 | Donors | PARTIAL | Installed versus candidate state explicit; current pins remain per consumer. |
| A017 | Qualification tree | CHECKED | Removed as authority. |
| A018 | Package/issue docs | CHECKED for R042 set | Source/runtime/prep/enricher/supervisor/overlay currentness and maintenance ownership corrected. |
| A019 | Issues/PR handoff | PENDING PUBLICATION SYNC | Update #336, #352 and PR #610 to the exact R042 SHA before ref advancement. |
| A020 | Runtime evidence | RUNTIME-ONLY | No current Windows/Qdrant/reconciliation/regex/PDF/MCP/overlay/performance result. |
| A021 | Compiler baseline | PARTIAL | R031 covers materializer+unitizer only. |
| A022 | Runtime identity/qualification | PARTIAL | Dormant worker/fixture code remains. |
| A023 | GC/storage diagnostics | PARTIAL | Ownership corrected; implementation unfinished. |
| A024 | Backup/restore | PARTIAL | Archive/import implementation absent. |
| A025 | Features/harness topology | PARTIAL | Remaining wave/query/proof/dormant wiring needs current caller cuts. |

## Evidence index

| Pass | Topic | Immutable record |
|---|---|---|
| R001–R012 | Governance/packages; lexical/source/preparation/publication/evidence | [44d7416c](https://github.com/UnknownAlienHuman/eliot-search/blob/44d7416c46554456f16ccd2b3943657b755304ee/docs/audit/ANALYSIS_REGISTRY.md) |
| R013–R020 | Client/runtime/lifecycle removals | [e6c7bb90](https://github.com/UnknownAlienHuman/eliot-search/blob/e6c7bb902c9aa91509ceeccae996c610f2b8f0cd/docs/audit/ANALYSIS_REGISTRY.md) |
| R021–R026 | Diagnostics/currentness/recovery/native authority | [5af8cde5](https://github.com/UnknownAlienHuman/eliot-search/blob/5af8cde5416fa33e3f1a8f3c19367ed520eac9a4/docs/audit/ANALYSIS_REGISTRY.md) |
| R027–R029 | Reconciliation, composition and config catalogue | [fb18eb96](https://github.com/UnknownAlienHuman/eliot-search/blob/fb18eb96b13948ac8abbb2a68996b81080aeef88/docs/audit/ANALYSIS_REGISTRY.md#r029-configuration-catalogue-removal-and-typed-input-policy) |
| R030 | Evaluation package removal | [e3b40a6f](https://github.com/UnknownAlienHuman/eliot-search/blob/e3b40a6f840177e734b43d925e3992581acdd436/docs/audit/ANALYSIS_REGISTRY.md#r030-remove-disconnected-evaluation-authority) |
| R031 | Markdown dependency/identity | [da382b66](https://github.com/UnknownAlienHuman/eliot-search/blob/da382b66e59d0b44e758bad9b22e2f5703a4e8c2/docs/audit/ANALYSIS_REGISTRY.md#r031-adopt-maintained-markdown-offsetiter-correction) |
| R032 | Canonical documentation | [3a5d3032](https://github.com/UnknownAlienHuman/eliot-search/blob/3a5d30322466d40e78f501d324d5aa7a4cb17362/docs/audit/ANALYSIS_REGISTRY.md#r032-canonical-documentation-and-index-reconciliation) |
| R033 | Package entry docs | [95c5f65c](https://github.com/UnknownAlienHuman/eliot-search/blob/95c5f65ce8544cb14c08936993cb826f8cab4d9d/docs/audit/ANALYSIS_REGISTRY.md#r033-package-entry-documentation-recheck) |
| R034 | Baseline inventory labels | [877ac38d](https://github.com/UnknownAlienHuman/eliot-search/blob/877ac38ddbd9aae4709eeb4661d711ab68b6feae/docs/audit/ANALYSIS_REGISTRY.md#r034-baseline-inventory-label-reconciliation) |
| R035 | Active issue evidence/currentness | [fbf2c55f](https://github.com/UnknownAlienHuman/eliot-search/blob/fbf2c55f6e4322a1221fc9963058861adc3c8c20/docs/audit/ANALYSIS_REGISTRY.md#r035-active-issue-handoff-reconciliation) |
| R036 | Completed removals/PDF donor | [b6e70f71](https://github.com/UnknownAlienHuman/eliot-search/blob/b6e70f716d855a435d9a0bef8de5086de0a03867/docs/audit/ANALYSIS_REGISTRY.md#r036-completed-removal-and-pdf-donor-handoff-reconciliation) |
| R037 | Watcher prerequisite/donor | [e1b66223](https://github.com/UnknownAlienHuman/eliot-search/blob/e1b66223ff255349b18135fb810a13afaa496ed6/docs/audit/ANALYSIS_REGISTRY.md#r037-watcher-prerequisite-and-donor-state) |
| R038 | Optional regex dependency | [d3d81ca0](https://github.com/UnknownAlienHuman/eliot-search/blob/d3d81ca0fc47a68c03235f21b276af0f46fb64f3/docs/audit/ANALYSIS_REGISTRY.md#r038-optional-regex-dependency-state) |
| R039 | Optional MCP SDK state | [0a9aa731](https://github.com/UnknownAlienHuman/eliot-search/blob/0a9aa73186eb7b7fb0c25c35d9ba947c2ba8fd4f/docs/audit/ANALYSIS_REGISTRY.md#r039-optional-mcp-sdk-state) |
| R040 | Document-worker candidate state | [8a63e7ad](https://github.com/UnknownAlienHuman/eliot-search/blob/8a63e7ad770ea4d57f506c54f07716bb06203111/docs/audit/ANALYSIS_REGISTRY.md#r040-document-worker-candidate-state) |
| R041 | Adapter/model-worker currentness and donor checkpoint | [6de4d2b3](https://github.com/UnknownAlienHuman/eliot-search/blob/6de4d2b3788b94714b59351cdb5dc5d0f0d13fd5/docs/audit/ANALYSIS_REGISTRY.md#r041-revision-currentness-and-donor-checkpoint) |
| R042 | Package currentness and maintenance ownership | [this pass](#r042-package-currentness-and-maintenance-ownership) |

## Execution evidence

- Run 39872058276 covers `search-qdrant-bridge --lib` at old commit `7d92fb8b` only.
- Run 40084893130 covers locked check/minimal Clippy for materializer + unitizer at R031 only.
- Current binaries, Windows, live Qdrant, reconciliation/watcher, regex, PDF, MCP, model/overlay integration,
  parser replay, recovery, relevance and scale remain unrun.
- R032–R042 are documentation-only and add no Rust/runtime result.

<a id="r042-package-currentness-and-maintenance-ownership"></a>

## R042. Package currentness and maintenance ownership

**Source:** `6de4d2b3788b94714b59351cdb5dc5d0f0d13fd5`, 2026-10-24.

**Inspected:** source family README/AGENTS; runtime family README/AGENTS and stopped-root maintenance owner;
preparation README/AGENTS; code-enricher README/AGENTS; Qdrant-supervisor README/AGENTS and #334; overlay
README/AGENTS and #336; query README; active documentation index. No private #598 source was read.

**Findings:** source AGENTS called the 1.22-million-byte baseline inventory “current” despite its README already
labelling it historical. Runtime AGENTS implied that current/pending/pinned roots replace local GC, conflicting
with the selected stopped-root maintenance owner and separate publication/Qdrant cleanup. Preparation,
code-enricher, supervisor and overlay instructions repeated inspected no-caller/size findings as permanent
current facts, while their issues or package READMEs already required a current caller search.

**Decision/change:** label size/caller findings as revision-specific throughout; require exact current
manifest/Rust/non-Rust/feature/target/test/workflow searches before package deletion; keep dormant-package
deletion rather than scaffold hardening when the current search confirms no consumer. Correct runtime ownership:
removed retention package stays removed, stopped-root maintenance owns local object collection/archive/import,
and publication owns Qdrant retirement.

**Unrun/remaining:** this pass does not execute the current caller searches because no package is deleted here.
No Rust, Cargo, workflow or runtime files change. Continue semantic review of remaining active package and issue
instructions; full documentation closure is not claimed.
