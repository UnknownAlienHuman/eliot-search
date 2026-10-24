# Analysis registry

Continuation memory for the current refactor. It records inspected evidence, decisions, published changes and
unrun work. It is not architecture, a permission system or a reason to delay known fixes.

## Resume after interruption

**Checkpoint:** R032, 2026-10-24 — canonical documentation and indexes reconciled.
**Branch:** `codex/refactor-preparation-20261017`, [PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610).
**Inspected head:** `da382b66e59d0b44e758bad9b22e2f5703a4e8c2` (R031).
**Original source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`.

R032 rebuilds the documentation entry path and corrects stale R031/#441, Qdrant-gate, test-evidence and audit
anchors. It changes documentation only. The persistent source → retained evidence → Qdrant → CLI application is
still unconnected.

Read [docs/README](../README.md), then the affected owner recipe and actual callers. Do not repeat R027 native/
config/test work, R028 composition/CLI caller mapping, R029 config-catalogue analysis, R030 evaluator search or
R031 Markdown dependency inspection as a new repository-wide audit.

**Next implementation:** #333 typed settings and real consumers, then #270/#330/#331 source/object/preparation
and #264 the connected product path. #441 is closed as the Markdown dependency/identity cut; product-level
parser/materialize/replay evidence remains #215/#331 work. Coordinate only actual #598 object/key overlap.

Current workspace has **49 explicit members**. This is inventory, not a target package count.

## Established decisions — do not rediscover

- Documentation authority is: architecture → implementation → nearest `FUNCTIONS.md`; product docs own
  workflows, registry owns evidence/status, donor register owns upstream facts.
- One current-format application owns root, source/object, preparation, publication, query and local session.
- Real root exclusion, source-currentness helpers and shared protected storage survive obsolete native/
  registration callers; the provider-registration platform does not.
- Config startup/status, locator hints and narrow DEK selection are the mapped legacy config consumers. Typed
  settings must reach them directly; receipts do not apply values.
- Query/publication/rebuild composition still uses oracle/in-memory/supplied-proof paths rather than selected
  live Qdrant/redb effects. Feature names do not connect them.
- All sixteen tests in the two mixed migration suites have receiving-owner/delete dispositions.
- Exact, orientation, comparison, documents and Git remain product requirements.
- R031 pins `pulldown-cmark 0.13.5`, its published checksum and identical materializer donor identity. Defaults
  remain disabled and dependencies remain `bitflags`, `memchr`, `unicase`.
- Repository config catalogues and `search-eval` were disconnected and are removed; do not recreate forwarding
  facades or status authorities.

## Delivery remaining

| Group | Current owners/issues | Missing product result |
|---|---|---|
| Invocation/root/settings/source/preparation | [implementation owners](../implementation/README.md#owners), #333/#266/#270/#330/#331/#271 | Validated values reach owners; retained current head and recoverable preparation. |
| Identity/Qdrant/publication | #259/#329/#263/#261/#300/#303 | Real SDK effects, atomic evidence/view, restartable rebuild and finite cleanup. |
| Query/evidence/window | [query](../../crates/search-query/README.md), #272/#279/#291/#293/#275/#249 | Scoped retrieval/Exact, pinned retained evidence and one bounded live-session window. |
| Protocol/client/CLI | #280/#332/#264 | One serviced framed child session and useful default CLI result. |
| Maintenance/archive/import | #302/#244 | Stopped-root collection, complete archive and fresh-destination import. |
| Product depth | [product behavior](../product/AGENT_ANALYSIS_FRAMEWORK.md), #281/#227/#335 | Source-backed orientation/comparison/documents/Git behavior. |

Optional evaluation (#233), MCP (#219), managed Qdrant (#334) and overlays (#336) follow a working baseline and
do not form additional product subsystems.

## Coverage ledger

`CHECKED` applies only to the named question. `PARTIAL` means known inspection/implementation remains.
`PROTECTED` excludes private work. `RUNTIME-ONLY` requires execution.

| ID | Area | Status | Current result / remaining |
|---|---|---|---|
| A001 | Documentation authority | CHECKED for canonical routing | R032 aligns root/docs/product/handoff/audit indexes and owner hierarchy; future owner changes must update their links. |
| A002 | Historical governance | CHECKED | Waves/packets/swarm/generated authority removed; Git preserves history. |
| A003 | Workflows/wrappers | CHECKED | Old gates/wrappers removed; Actions remain manual-only. |
| A004 | Workspace inventory | CHECKED for inventory | 49 members; R031 resolved/compiled materializer+unitizer only; full workspace/binaries remain unrun. |
| A005 | Entrypoints/product | PARTIAL | Active CLI/daemon shadowing mapped; real application absent. |
| A006 | Root/config/control | PARTIAL | Consumers/layer policy known; typed settings/root/current state not implemented. |
| A007 | Sources/reconciliation | PARTIAL | Acquisition/currentness defects known; retained-source integration unfinished. |
| A008 | Private object/key work | PROTECTED | Consume accepted #598 handoff only. |
| A009 | Preparation/identity | PARTIAL | Markdown parser/identity updated; budget/empty/stale/ABA/reuse integration remains. |
| A010 | Qdrant/donors | CHECKED for named static questions | BM25/tokenizer/IDF/readback/late-write limits recorded; live integration/quality/scale unrun. |
| A011 | Publication/view/cleanup | PARTIAL | Recovery/lifetime boundaries selected; real effects/current-view path absent. |
| A012 | Query/evidence/window | PARTIAL | Oracle/lock/cost defects mapped; connected path absent. |
| A013 | Protocol/client/CLI | PARTIAL | Legacy seams mapped; serviced current session absent. |
| A014 | Product depth | PARTIAL | Requirements preserved; implementations incomplete. |
| A015 | Tests/governance code | PARTIAL | Mixed suites classified; remaining wave/query/lexical fixtures need caller cutover. |
| A016 | Donors | PARTIAL | Markdown 0.13.5 installed; other selected donors remain per concrete consumer. |
| A017 | Qualification tree | CHECKED | Archived/removed as authority; runtime evidence remains ordinary work. |
| A018 | Auxiliary docs | CHECKED for current index | R032 classifies canonical versus historical records and fixes stale anchors/status. |
| A019 | Issues/PR handoff | CHECKED for R032 | #352, PR #610 and the stale #189 evidence-index link were synchronized after publication. |
| A020 | Runtime evidence | RUNTIME-ONLY | Static docs/removals establish no Windows/Qdrant/recovery/performance result. |
| A021 | Compiler baseline | PARTIAL | R031 check/Clippy passed materializer+unitizer; current binaries/full workspace unchecked. |
| A022 | Runtime identity/qualification | PARTIAL | Markdown donor identity truthful; lexical/materializer fixture authority still needs cleanup. |
| A023 | GC/storage diagnostics | PARTIAL | Stopped-root design selected; implementation unfinished. |
| A024 | Backup/restore | PARTIAL | Fresh archive/import design selected; implementation absent. |
| A025 | Features/harness topology | PARTIAL | Eval and wave7 removed; remaining wave/query/proof wiring needs real caller cuts. |

## Evidence index

Full earlier records remain immutable. These links preserve detailed readsets without re-expanding this live
registry.

| Pass | Topic | Immutable record |
|---|---|---|
| R001–R012 | Governance/packages; lexical/source/preparation/publication/evidence | [44d7416c](https://github.com/UnknownAlienHuman/eliot-search/blob/44d7416c46554456f16ccd2b3943657b755304ee/docs/audit/ANALYSIS_REGISTRY.md) |
| R013 | Client display/cancellation/framing/cleanup | [2b37f5ef](https://github.com/UnknownAlienHuman/eliot-search/blob/2b37f5ef1e8cc755bbcd8c0e1f7efc840eff8cf3/docs/audit/ANALYSIS_REGISTRY.md) |
| R014 | Query inventories, local GC, restore isolation | [8cf331a4](https://github.com/UnknownAlienHuman/eliot-search/blob/8cf331a44decded2c84c7f579dcbf8434e040dc7/docs/audit/ANALYSIS_REGISTRY.md) |
| R015–R020 | Structural/lifecycle removals through retention shell/features | [e6c7bb90](https://github.com/UnknownAlienHuman/eliot-search/blob/e6c7bb902c9aa91509ceeccae996c610f2b8f0cd/docs/audit/ANALYSIS_REGISTRY.md) |
| R021–R026 | Diagnostics/currentness/recovery/native authority | [5af8cde5](https://github.com/UnknownAlienHuman/eliot-search/blob/5af8cde5416fa33e3f1a8f3c19367ed520eac9a4/docs/audit/ANALYSIS_REGISTRY.md) |
| R027 | Native/config/test/product/donor reconciliation | [37dbb198](https://github.com/UnknownAlienHuman/eliot-search/blob/37dbb198ca983d00e987f17ed2a71171d53d1d4e/docs/audit/ANALYSIS_REGISTRY.md#r027-owner-consumer-and-donor-reconciliation) |
| R028 | Composition callers, CLI orphans, Markdown fix research | [afc3bfca](https://github.com/UnknownAlienHuman/eliot-search/blob/afc3bfca60cd0a350c3cf822ccda48f5a13e2028/docs/audit/ANALYSIS_REGISTRY.md#r028-composition-cutover-and-maintained-markdown-fix) |
| R029 | Config catalogue removal and typed input policy | [fb18eb96](https://github.com/UnknownAlienHuman/eliot-search/blob/fb18eb96b13948ac8abbb2a68996b81080aeef88/docs/audit/ANALYSIS_REGISTRY.md#r029-configuration-catalogue-removal-and-typed-input-policy) |
| R030 | Disconnected evaluation package removal | [e3b40a6f](https://github.com/UnknownAlienHuman/eliot-search/blob/e3b40a6f840177e734b43d925e3992581acdd436/docs/audit/ANALYSIS_REGISTRY.md#r030-remove-disconnected-evaluation-authority) |
| R031 | Markdown 0.13.5 dependency/identity cut | [da382b66](https://github.com/UnknownAlienHuman/eliot-search/blob/da382b66e59d0b44e758bad9b22e2f5703a4e8c2/docs/audit/ANALYSIS_REGISTRY.md#r031-adopt-maintained-markdown-offsetiter-correction) |
| R032 | Canonical documentation/index reconciliation | [this pass](#r032-canonical-documentation-and-index-reconciliation) |

Supporting historical evidence: [distilled audit](ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md),
[baseline member disposition](WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md) and the
[superseded coverage pointer](AUDIT_COVERAGE_AND_DOCUMENTATION_MAP_2026-10-21.md).

## Execution evidence

- [Run 39872058276](https://github.com/UnknownAlienHuman/eliot-search/actions/runs/39872058276): Ubuntu,
  Rust 1.98, `search-qdrant-bridge --lib`, commit `7d92fb8b`; no consumers/runtime.
- [Run 40084893130](https://github.com/UnknownAlienHuman/eliot-search/actions/runs/40084893130): Rust 1.98 locked
  check and minimal Clippy for `search-materializer` + `search-unitizer` at the R031 cut.
- Parser reproducer/materialize/replay, current binaries, Windows, live Qdrant, restart/recovery, relevance and
  scale remain unrun.
- R032 is documentation-only. Its verification is link/anchor/status consistency and GitHub diff review; it
  adds no Rust/runtime result.

## R030. Remove disconnected evaluation authority

At source `fb18eb96`, `search-eval` had no external Rust caller; its daemon edge was Cargo-only. R030 removed the
41-file package, workspace/feature/dependency wiring and exact local lock entries. Future evaluation under #233
may be a small post-product tool over explicit finite result files; no facade, verdict authority or runtime gate
remains.

## R031. Adopt maintained Markdown OffsetIter correction

At source `2a04c1dd`, the materializer was the sole installed production consumer and still pinned 0.13.1.
R031 changed the workspace/lock/parser/donor identity together to 0.13.5 and its published checksum, preserving
empty options, `OffsetIter`, exact retained bytes and parser behavior in transform identity. Run 40084893130
passed the owner/consumer check and Clippy. #441 is closed as this code task; product-level parser cases remain
#215/#331 evidence.

## R032. Canonical documentation and index reconciliation

**Source:** `da382b66e59d0b44e758bad9b22e2f5703a4e8c2`, 2026-10-24.

**Inspected:** root README/QUICKSTART/AGENTS; complete `docs/` inventory; architecture, implementation, product,
handoff, config, runtime and ADR entrypoints; live registry/donor/test records; bridge owner map; PR #610 and
issues #441/#352/#333 plus stale issue links found during navigation review. No private #598 source was read.

**Findings:** the canonical architecture and product behavior were broadly consistent, but entry pages were
history-heavy and lacked one clear reading/delivery sequence. R031 left #441 described as open in the registry
and donor register after the issue closed. The Qdrant runtime boundary still claimed `QualifiedGate` was a
constructor input although the gate had already been removed. Test-cutover execution headers stopped at R027,
the old coverage pointer linked a deleted registry anchor, and the Windows secret page read as target design
instead of a legacy effect inventory.

**Decision/change:** rebuild root/docs/product/handoff navigation around the authority chain; add an explicit
implementation delivery sequence and completion boundary; refresh test/evidence status; classify the Windows
adapter as current legacy inventory; correct the Qdrant gate/remaining-debt description; rewrite donor and
analysis registers into concise live indexes while preserving immutable historical links; add an ADR index.
No product requirement, source evidence or #598 protection is removed.

**Publication follow-up completed:** #352, PR #610 and the stale #189 evidence-index link were synchronized from this record. No second documentation authority was created. The product implementation and all runtime evidence listed above remain outstanding.
