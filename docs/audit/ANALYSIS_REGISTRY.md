# Analysis registry

Continuation memory for the current refactor. It records inspected evidence, decisions, published changes and
unrun work. It is not architecture, a permission system or a reason to delay known fixes.

## Resume after interruption

**Checkpoint:** R037, 2026-10-24 — watcher prerequisite and donor state corrected.  
**Branch:** `codex/refactor-preparation-20261017`, [PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610).  
**Inspected head:** `b6e70f716d855a435d9a0bef8de5086de0a03867` (R036).  
**Original source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`.

R037 corrects #239: optional filesystem notifications cannot be implemented over the current unavailable sync
path. #271 still records `SOURCE_ROOT_SYNC_EFFECTS_UNAVAILABLE`; startup/explicit/periodic reconciliation must
first converge without notifications. `notify` is absent from the current lock and remains an implementation-time
candidate whose exact version/MSRV/features/platform backends/advisories move with the real adapter.

The persistent source → retained evidence → Qdrant → CLI application remains unconnected. Read
[docs/README](../README.md), then the affected owner recipe and exact current callers. Do not repeat R027–R037
as another repository-wide audit.

**Next implementation:** #333 typed settings and real consumers, then #270/#330/#331 source/object/preparation
and #264 the connected product path. #239 stays optional behind actual #271 completion. Coordinate only actual
#598 object/key overlap.

Current workspace has **49 explicit members**. This is inventory, not a target package count.

## Established decisions — do not rediscover

- Documentation authority is architecture → implementation → nearest `FUNCTIONS.md`; product docs own
  workflows, this registry owns evidence/status, and the donor register owns upstream facts.
- One current-format application owns root, source/object, preparation, publication, query and local session.
- Config receipts do not apply values. Typed settings must reach actual root/source/Qdrant/query consumers.
- Query/publication/rebuild composition still contains disconnected oracle/in-memory/supplied-proof paths.
  Feature names, constructors and tests do not connect them.
- Exact, orientation, comparison, documents and Git remain product requirements.
- Historical size/caller measurements are tied to their inspected baseline; exact current callers are checked
  inside the implementation slice.
- Completed removals are not active tasks. `search-retention`, `wave7-lifecycle` and `full-baseline` were already
  removed; remaining maintenance documentation describes the replacement behavior.
- Inspected donors are not installed dependencies. Exact version, features, MSRV, advisories and native artifact
  are verified and pinned with the current consumer.
- A watcher is only a latency hint after source reconciliation works independently. It never establishes source
  identity, bytes, deletion, currentness or completeness.
- R031 pins `pulldown-cmark 0.13.5` and matching materializer identity; #441 is closed as that dependency task.
- #598 private object/key work remains protected; consume only its accepted public handoff.

## Delivery remaining

| Group | Current owners/issues | Missing product result |
|---|---|---|
| Invocation/root/settings/source/preparation | [implementation owners](../implementation/README.md#owners), #333/#266/#270/#330/#331/#271 | Validated settings reach owners; retained current head and recoverable reconciliation/preparation. |
| Identity/Qdrant/publication | #259/#329/#263/#261/#300/#303 | Real SDK effects, atomic evidence/view, restartable rebuild and finite cleanup. |
| Query/evidence/window | [query](../../crates/search-query/README.md), #272/#279/#291/#293/#275/#249 | Scoped retrieval/Exact, pinned retained evidence and one bounded live-session window. |
| Protocol/client/CLI | #280/#332/#264 | One serviced framed child session and useful default CLI result. |
| Maintenance/archive/import | #302/#244 | Stopped-root collection, complete archive and fresh-destination import. |
| Product depth | [product behavior](../product/AGENT_ANALYSIS_FRAMEWORK.md), #281/#227/#335 | Source-backed orientation/comparison/documents/Git behavior. |

Optional watcher (#239), evaluation (#233), MCP (#219), managed Qdrant (#334), overlays (#336) and models follow
working prerequisites and do not form baseline subsystems.

## Coverage ledger

| ID | Area | Status | Current result / remaining |
|---|---|---|---|
| A001 | Documentation authority | CHECKED for R032–R037 scope | Canonical indexes, package/issue handoffs and donor state are aligned. |
| A002 | Historical governance | CHECKED | Waves/packets/swarm/generated authority removed; Git preserves history. |
| A003 | Workflows/wrappers | CHECKED | Old gates/wrappers removed; Actions remain manual-only. |
| A004 | Workspace inventory | CHECKED for inventory | 49 members; current full workspace/binaries remain unrun. |
| A005 | Entrypoints/product | PARTIAL | CLI/daemon shadowing mapped; real application absent. |
| A006 | Root/config/control | PARTIAL | Typed settings/root/current state not implemented. |
| A007 | Sources/reconciliation | PARTIAL | Actual sync entrypoint unavailable; watcher is correctly deferred. |
| A008 | Private object/key work | PROTECTED | Consume accepted #598 handoff only. |
| A009 | Preparation/identity | PARTIAL | Markdown parser/identity updated; connected preparation remains. |
| A010 | Qdrant/donors | CHECKED for named static questions | Live integration/quality/scale unrun. |
| A011 | Publication/view/cleanup | PARTIAL | Real effects/current-view path absent. |
| A012 | Query/evidence/window | PARTIAL | Connected path absent. |
| A013 | Protocol/client/CLI | PARTIAL | Serviced current session absent. |
| A014 | Product depth | PARTIAL | Requirements preserved; implementations incomplete. |
| A015 | Tests/governance code | PARTIAL | Remaining wave/query/lexical fixtures need caller cutover. |
| A016 | Donors | PARTIAL | Installed versus candidate state is explicit; current consumer pins remain future work. |
| A017 | Qualification tree | CHECKED | Removed as authority; runtime evidence remains ordinary work. |
| A018 | Package/issue docs | CHECKED for R037 set | Watcher prerequisite and donor status corrected. |
| A019 | Issues/PR handoff | CHECKED by publication sequence | #239 corrected before this commit; synchronize PR/#352/#214 to exact commit before branch update. |
| A020 | Runtime evidence | RUNTIME-ONLY | No current Windows/Qdrant/recovery/watcher/performance result. |
| A021 | Compiler baseline | PARTIAL | R031 check/Clippy covers materializer+unitizer only. |
| A022 | Runtime identity/qualification | PARTIAL | Dormant worker/fixture code still needs caller-based cleanup. |
| A023 | GC/storage diagnostics | PARTIAL | Stopped-root design selected; implementation unfinished. |
| A024 | Backup/restore | PARTIAL | Fresh archive/import design selected; implementation absent. |
| A025 | Features/harness topology | PARTIAL | Remaining wave/query/proof and dormant-worker wiring need real caller cuts. |

## Evidence index

| Pass | Topic | Immutable record |
|---|---|---|
| R001–R012 | Governance/packages; lexical/source/preparation/publication/evidence | [44d7416c](https://github.com/UnknownAlienHuman/eliot-search/blob/44d7416c46554456f16ccd2b3943657b755304ee/docs/audit/ANALYSIS_REGISTRY.md) |
| R013–R020 | Client/runtime/lifecycle removals | [e6c7bb90](https://github.com/UnknownAlienHuman/eliot-search/blob/e6c7bb902c9aa91509ceeccae996c610f2b8f0cd/docs/audit/ANALYSIS_REGISTRY.md) |
| R021–R026 | Diagnostics/currentness/recovery/native authority | [5af8cde5](https://github.com/UnknownAlienHuman/eliot-search/blob/5af8cde5416fa33e3f1a8f3c19367ed520eac9a4/docs/audit/ANALYSIS_REGISTRY.md) |
| R027 | Native/config/test/product/donor reconciliation | [37dbb198](https://github.com/UnknownAlienHuman/eliot-search/blob/37dbb198ca983d00e987f17ed2a71171d53d1d4e/docs/audit/ANALYSIS_REGISTRY.md#r027-owner-consumer-and-donor-reconciliation) |
| R028 | Composition callers, CLI orphans, Markdown research | [afc3bfca](https://github.com/UnknownAlienHuman/eliot-search/blob/afc3bfca60cd0a350c3cf822ccda48f5a13e2028/docs/audit/ANALYSIS_REGISTRY.md#r028-composition-cutover-and-maintained-markdown-fix) |
| R029 | Config catalogue removal and typed input policy | [fb18eb96](https://github.com/UnknownAlienHuman/eliot-search/blob/fb18eb96b13948ac8abbb2a68996b81080aeef88/docs/audit/ANALYSIS_REGISTRY.md#r029-configuration-catalogue-removal-and-typed-input-policy) |
| R030 | Evaluation package removal | [e3b40a6f](https://github.com/UnknownAlienHuman/eliot-search/blob/e3b40a6f840177e734b43d925e3992581acdd436/docs/audit/ANALYSIS_REGISTRY.md#r030-remove-disconnected-evaluation-authority) |
| R031 | Markdown 0.13.5 dependency/identity | [da382b66](https://github.com/UnknownAlienHuman/eliot-search/blob/da382b66e59d0b44e758bad9b22e2f5703a4e8c2/docs/audit/ANALYSIS_REGISTRY.md#r031-adopt-maintained-markdown-offsetiter-correction) |
| R032 | Canonical documentation/index reconciliation | [3a5d3032](https://github.com/UnknownAlienHuman/eliot-search/blob/3a5d30322466d40e78f501d324d5aa7a4cb17362/docs/audit/ANALYSIS_REGISTRY.md#r032-canonical-documentation-and-index-reconciliation) |
| R033 | Active package entry documentation | [95c5f65c](https://github.com/UnknownAlienHuman/eliot-search/blob/95c5f65ce8544cb14c08936993cb826f8cab4d9d/docs/audit/ANALYSIS_REGISTRY.md#r033-package-entry-documentation-recheck) |
| R034 | Baseline inventory labels | [877ac38d](https://github.com/UnknownAlienHuman/eliot-search/blob/877ac38ddbd9aae4709eeb4661d711ab68b6feae/docs/audit/ANALYSIS_REGISTRY.md#r034-baseline-inventory-label-reconciliation) |
| R035 | Active issue evidence/currentness | [fbf2c55f](https://github.com/UnknownAlienHuman/eliot-search/blob/fbf2c55f6e4322a1221fc9963058861adc3c8c20/docs/audit/ANALYSIS_REGISTRY.md#r035-active-issue-handoff-reconciliation) |
| R036 | Completed removals and PDF donor boundary | [b6e70f71](https://github.com/UnknownAlienHuman/eliot-search/blob/b6e70f716d855a435d9a0bef8de5086de0a03867/docs/audit/ANALYSIS_REGISTRY.md#r036-completed-removal-and-pdf-donor-handoff-reconciliation) |
| R037 | Watcher prerequisite and donor state | [this pass](#r037-watcher-prerequisite-and-donor-state) |

Supporting historical evidence: [distilled audit](ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md),
[baseline member disposition](WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md) and the
[superseded coverage pointer](AUDIT_COVERAGE_AND_DOCUMENTATION_MAP_2026-10-21.md).

## Execution evidence

- [Run 39872058276](https://github.com/UnknownAlienHuman/eliot-search/actions/runs/39872058276): Rust 1.98
  `search-qdrant-bridge --lib` at `7d92fb8b`; no consumers/runtime.
- [Run 40084893130](https://github.com/UnknownAlienHuman/eliot-search/actions/runs/40084893130): Rust 1.98 locked
  check/minimal Clippy for `search-materializer` + `search-unitizer` at R031.
- Current binaries, Windows, live Qdrant, parser replay, source reconciliation/watcher, restart/recovery,
  relevance and scale remain unrun.
- R032–R037 are documentation-only and add no Rust/runtime result.

<a id="r037-watcher-prerequisite-and-donor-state"></a>

## R037. Watcher prerequisite and donor state

**Source:** `b6e70f716d855a435d9a0bef8de5086de0a03867`, 2026-10-24.

**Inspected:** #239 and #271 issue bodies; current reconciliation owner reference; current `Cargo.lock`; live donor
register and documentation index. No private #598 source was read.

**Finding:** #239 said “#271 already works without notifications” while #271 explicitly records that the actual
sync entrypoint ends in `SOURCE_ROOT_SYNC_EFFECTS_UNAVAILABLE`. The current lock contains no `notify` package.
Starting watcher work now would build responsiveness plumbing over a missing ingestion/reconciliation baseline.

**Decision/change:** #239 now blocks implementation until #271 demonstrates startup, explicit sync and periodic
convergence without notifications. Notifications remain non-authoritative hints. `notify` is recorded as an
uninstalled candidate; its exact version, API, MSRV, features, platform backends and advisories are selected and
pinned only with the actual adapter after the prerequisite works. The donor register and docs index are updated.

**Publication sequence:** #239 was corrected before this tree commit. PR #610, #352 and #214 are synchronized to
the exact commit object before the branch ref advances. Product implementation and all runtime evidence remain
outstanding.
