# Analysis registry

Continuation memory for the current refactor. It records inspected evidence, decisions, published changes and
unrun work. It is not architecture, a permission system or a reason to delay known fixes.

## Resume after interruption

**Checkpoint:** R040, 2026-10-24 — document-worker donor/package state aligned.  
**Branch:** `codex/refactor-preparation-20261017`, [PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610).  
**Inspected head:** `0a9aa73186eb7b7fb0c25c35d9ba947c2ba8fd4f` (R039).  
**Original source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`.

R040 aligns the document-worker package entry docs with #227 and the donor register. The current package remains
a refusal stub with an unlinked custom SHA-256 and unused internal dependencies. `pdfium-render`/PDFium is absent
from the current lock/runtime and is an implementation-time candidate boundary, not an installed/frozen donor.

The persistent source → retained evidence → Qdrant → CLI application remains unconnected. Read
[docs/README](../README.md), then the affected owner recipe and exact current callers. Do not repeat R027–R040
as another repository-wide audit.

**Next implementation:** #333 typed settings and real consumers, then #270/#330/#331/#271
source/object/preparation/reconciliation and #264 the connected product path. Optional document/MCP/regex/
watcher/model work remains behind its stated prerequisite and implementation-time donor selection. Coordinate
only actual #598 overlap.

Current workspace has **49 explicit members**. This is inventory, not a target package count.

## Established decisions — do not rediscover

- Documentation authority is architecture → implementation → nearest `FUNCTIONS.md`; product docs own
  workflows, this registry owns evidence/status, and the donor register owns upstream facts.
- One current-format application owns root, source/object, preparation, publication, query and local session.
- Config receipts do not apply values; typed settings must reach actual consumers.
- Query/publication/rebuild composition still contains disconnected oracle/in-memory/supplied-proof paths.
- Historical size/caller measurements are baseline evidence, not current facts or deletion targets.
- Completed removals are not active tasks. Remaining documentation describes replacement behavior.
- An inspected donor is not installed. Exact version, features, MSRV, advisories and native artifact move with
  the real consumer implementation.
- Literal Exact is baseline; optional regex cannot block it or reserve a speculative dependency.
- Reconciliation must work independently before optional watcher hints.
- Standalone Search must work before optional MCP; `rmcp` remains absent.
- Document support is not the refusal worker. PDFium remains an uninstalled candidate requiring one exact
  current crate plus matching packaged native artifact with implementation.
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

Optional document (#227), MCP (#219), regex (#248), watcher (#239), evaluation (#233), managed Qdrant (#334),
overlays (#336) and models follow working prerequisites and do not form baseline subsystems.

## Coverage ledger

| ID | Area | Status | Current result / remaining |
|---|---|---|---|
| A001 | Documentation authority | CHECKED for R032–R040 scope | Canonical indexes, package/issue handoffs and donor states are aligned. |
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
| A012 | Query/evidence/window | PARTIAL | Connected path absent; optional regex remains uninstalled. |
| A013 | Protocol/client/CLI | PARTIAL | Serviced session absent. |
| A014 | Product depth | PARTIAL | Document worker remains a stub; other product-depth implementations incomplete. |
| A015 | Tests/governance code | PARTIAL | Remaining fixtures need caller cutover. |
| A016 | Donors | PARTIAL | Installed versus candidate state explicit; current pins remain per consumer. |
| A017 | Qualification tree | CHECKED | Removed as authority. |
| A018 | Package/issue docs | CHECKED for R040 set | Document-worker package now matches #227 and donor state. |
| A019 | Issues/PR handoff | CHECKED by publication sequence | #227 already current; PR/#352 synchronized to exact commit before ref update. |
| A020 | Runtime evidence | RUNTIME-ONLY | No current Windows/Qdrant/reconciliation/PDF/MCP/regex/performance result. |
| A021 | Compiler baseline | PARTIAL | R031 covers materializer+unitizer only. |
| A022 | Runtime identity/qualification | PARTIAL | Dormant adapter/worker/fixture code remains. |
| A023 | GC/storage diagnostics | PARTIAL | Implementation unfinished. |
| A024 | Backup/restore | PARTIAL | Archive/import implementation absent. |
| A025 | Features/harness topology | PARTIAL | Remaining wave/query/proof/dormant wiring needs current caller cuts. |

## Evidence index

| Pass | Topic | Immutable record |
|---|---|---|
| R001–R012 | Governance/packages; lexical/source/preparation/publication/evidence | [44d7416c](https://github.com/UnknownAlienHuman/eliot-search/blob/44d7416c46554456f16ccd2b3943657b755304ee/docs/audit/ANALYSIS_REGISTRY.md) |
| R013–R020 | Client/runtime/lifecycle removals | [e6c7bb90](https://github.com/UnknownAlienHuman/eliot-search/blob/e6c7bb902c9aa91509ceeccae996c610f2b8f0cd/docs/audit/ANALYSIS_REGISTRY.md) |
| R021–R026 | Diagnostics/currentness/recovery/native authority | [5af8cde5](https://github.com/UnknownAlienHuman/eliot-search/blob/5af8cde5416fa33e3f1a8f3c19367ed520eac9a4/docs/audit/ANALYSIS_REGISTRY.md) |
| R027–R029 | Reconciliation, composition and config catalogue work | [fb18eb96](https://github.com/UnknownAlienHuman/eliot-search/blob/fb18eb96b13948ac8abbb2a68996b81080aeef88/docs/audit/ANALYSIS_REGISTRY.md#r029-configuration-catalogue-removal-and-typed-input-policy) |
| R030 | Evaluation package removal | [e3b40a6f](https://github.com/UnknownAlienHuman/eliot-search/blob/e3b40a6f840177e734b43d925e3992581acdd436/docs/audit/ANALYSIS_REGISTRY.md#r030-remove-disconnected-evaluation-authority) |
| R031 | Markdown dependency/identity | [da382b66](https://github.com/UnknownAlienHuman/eliot-search/blob/da382b66e59d0b44e758bad9b22e2f5703a4e8c2/docs/audit/ANALYSIS_REGISTRY.md#r031-adopt-maintained-markdown-offsetiter-correction) |
| R032 | Canonical documentation | [3a5d3032](https://github.com/UnknownAlienHuman/eliot-search/blob/3a5d30322466d40e78f501d324d5aa7a4cb17362/docs/audit/ANALYSIS_REGISTRY.md#r032-canonical-documentation-and-index-reconciliation) |
| R033 | Package entry docs | [95c5f65c](https://github.com/UnknownAlienHuman/eliot-search/blob/95c5f65ce8544cb14c08936993cb826f8cab4d9d/docs/audit/ANALYSIS_REGISTRY.md#r033-package-entry-documentation-recheck) |
| R034 | Baseline inventory labels | [877ac38d](https://github.com/UnknownAlienHuman/eliot-search/blob/877ac38ddbd9aae4709eeb4661d711ab68b6feae/docs/audit/ANALYSIS_REGISTRY.md#r034-baseline-inventory-label-reconciliation) |
| R035 | Active issue evidence/currentness | [fbf2c55f](https://github.com/UnknownAlienHuman/eliot-search/blob/fbf2c55f6e4322a1221fc9963058861adc3c8c20/docs/audit/ANALYSIS_REGISTRY.md#r035-active-issue-handoff-reconciliation) |
| R036 | Completed removals/PDF donor | [b6e70f71](https://github.com/UnknownAlienHuman/eliot-search/blob/b6e70f716d855a435d9a0bef8de5086de0a03867/docs/audit/ANALYSIS_REGISTRY.md#r036-completed-removal-and-pdf-donor-handoff-reconciliation) |
| R037 | Watcher prerequisite/donor | [e1b66223](https://github.com/UnknownAlienHuman/eliot-search/blob/e1b66223ff255349b18135fb810a13afaa496ed6/docs/audit/ANALYSIS_REGISTRY.md#r037-watcher-prerequisite-and-donor-state) |
| R038 | Optional regex dependency state | [d3d81ca0](https://github.com/UnknownAlienHuman/eliot-search/blob/d3d81ca0fc47a68c03235f21b276af0f46fb64f3/docs/audit/ANALYSIS_REGISTRY.md#r038-optional-regex-dependency-state) |
| R039 | Optional MCP SDK state | [0a9aa731](https://github.com/UnknownAlienHuman/eliot-search/blob/0a9aa73186eb7b7fb0c25c35d9ba947c2ba8fd4f/docs/audit/ANALYSIS_REGISTRY.md#r039-optional-mcp-sdk-state) |
| R040 | Document-worker candidate state | [this pass](#r040-document-worker-candidate-state) |

## Execution evidence

- Run 39872058276 covers `search-qdrant-bridge --lib` at old commit `7d92fb8b` only.
- Run 40084893130 covers locked check/minimal Clippy for materializer + unitizer at R031 only.
- Current binaries, Windows, live Qdrant, reconciliation/watcher, PDF, MCP, regex, parser replay, recovery,
  relevance and scale remain unrun.
- R032–R040 are documentation-only and add no Rust/runtime result.

<a id="r040-document-worker-candidate-state"></a>

## R040. Document-worker candidate state

**Source:** `0a9aa73186eb7b7fb0c25c35d9ba947c2ba8fd4f`, 2026-10-24.

**Inspected:** current `Cargo.lock`; document-worker README/FUNCTIONS/AGENTS, manifest/source facts and #227;
live donor register. No private #598 source was read.

**Finding:** #227 and the donor register correctly treated `pdfium-render`/PDFium as an uninstalled
implementation-time candidate, but the package README/FUNCTIONS still used “selected donor” and direct “use
pdfium-render” wording which could be read as a frozen API/version. The current lock contains no PDFium package
and no matching native artifact is selected.

**Decision/change:** package README/FUNCTIONS/AGENTS now state that the refusal stub is not a capability and that
PDFium is only a candidate boundary. The real #227 slice must recheck and pin one current crate version plus one
matching packaged native artifact with API/MSRV/advisory/feature/license/hash/platform evidence, or remove the
package while deferred. Source identity remains the retained `ObjectRef`; derived text remains mapped page/region
evidence, not binary-byte identity.

**Publication sequence:** #227 already contained the correct candidate boundary and required no metadata change.
PR #610 and #352 are synchronized to the exact commit object before the branch ref advances. Product
implementation, exact native selection and all runtime evidence remain outstanding.
