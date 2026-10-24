# Analysis registry

Continuation memory for the current refactor. It records inspected evidence, decisions, published changes and
unrun work. It is not architecture, a permission system or a reason to delay known fixes.

## Resume after interruption

**Checkpoint:** R034, 2026-10-24 — historical package inventories separated from current branch status.  
**Branch:** `codex/refactor-preparation-20261017`, [PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610).  
**Inspected head:** `95c5f65ce8544cb14c08936993cb826f8cab4d9d` (R033).  
**Original source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`.

R034 corrects package documentation which still described byte counts, trait/operation totals, manifest counts
and caller censuses from the original baseline as “current” or “public main”. The measurements remain useful
historical evidence, but current callers must be rechecked at the actual code cut. R034 changes Markdown only.

The persistent source → retained evidence → Qdrant → CLI application is still unconnected. Read
[docs/README](../README.md), then the affected owner recipe and actual callers. Do not repeat R027 native/config/
test work, R028 composition/CLI caller mapping, R029 config-catalogue analysis, R030 evaluator search, R031
Markdown dependency inspection, R032 canonical documentation review or R033 package-entry review as another
general audit.

**Next implementation:** #333 typed settings and real consumers, then #270/#330/#331 source/object/preparation
and #264 the connected product path. #441 remains closed as the Markdown dependency/identity cut; connected
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
- CLI/daemon command presence is source reachability, not execution evidence. Current binaries remain
  uncompiled/unrun at this head.
- The model worker is a dormant refusal stub, not an optional capability or qualification gate. Its obsolete
  P15/G6 output is removal input, not current authority.
- Baseline source-size, trait, operation, manifest and caller-count measurements are historical facts tied to
  `c8e38b6`; they are not current branch facts, acceptance criteria or deletion targets.

## Delivery remaining

| Group | Current owners/issues | Missing product result |
|---|---|---|
| Invocation/root/settings/source/preparation | [implementation owners](../implementation/README.md#owners), #333/#266/#270/#330/#331/#271 | Validated values reach owners; retained current head and recoverable preparation. |
| Identity/Qdrant/publication | #259/#329/#263/#261/#300/#303 | Real SDK effects, atomic evidence/view, restartable rebuild and finite cleanup. |
| Query/evidence/window | [query](../../crates/search-query/README.md), #272/#279/#291/#293/#275/#249 | Scoped retrieval/Exact, pinned retained evidence and one bounded live-session window. |
| Protocol/client/CLI | #280/#332/#264 | One serviced framed child session and useful default CLI result. |
| Maintenance/archive/import | #302/#244 | Stopped-root collection, complete archive and fresh-destination import. |
| Product depth | [product behavior](../product/AGENT_ANALYSIS_FRAMEWORK.md), #281/#227/#335 | Source-backed orientation/comparison/documents/Git behavior. |

Optional evaluation (#233), MCP (#219), managed Qdrant (#334), overlays (#336) and models follow a working
baseline and do not form additional product subsystems.

## Coverage ledger

`CHECKED` applies only to the named question. `PARTIAL` means known inspection/implementation remains.
`PROTECTED` excludes private work. `RUNTIME-ONLY` requires execution.

| ID | Area | Status | Current result / remaining |
|---|---|---|---|
| A001 | Documentation authority | CHECKED for canonical/package routing | R032–R034 align canonical indexes, active entry docs and baseline/current labels. Future owner changes update their links. |
| A002 | Historical governance | CHECKED | Waves/packets/swarm/generated authority removed; Git preserves history. |
| A003 | Workflows/wrappers | CHECKED | Old gates/wrappers removed; Actions remain manual-only. |
| A004 | Workspace inventory | CHECKED for inventory | 49 members; R031 resolved/compiled materializer+unitizer only; full workspace/binaries remain unrun. |
| A005 | Entrypoints/product | PARTIAL | CLI/daemon shadowing mapped and documentation truthful; real application absent. |
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
| A018 | Auxiliary/package docs | CHECKED for R034 set | Current entry docs and historical baseline inventories are distinguished; local owner docs change only with concrete findings. |
| A019 | Issues/PR handoff | CHECKED for R034 | PR #610, #352, #214, #334 and #266 were synchronized after publication; #266's historical size was removed from its title. |
| A020 | Runtime evidence | RUNTIME-ONLY | Static docs/removals establish no Windows/Qdrant/recovery/performance result. |
| A021 | Compiler baseline | PARTIAL | R031 check/Clippy passed materializer+unitizer; current binaries/full workspace unchecked. |
| A022 | Runtime identity/qualification | PARTIAL | Markdown donor identity truthful; lexical/materializer fixture authority and dormant worker code still need cleanup. |
| A023 | GC/storage diagnostics | PARTIAL | Stopped-root design selected; implementation unfinished. |
| A024 | Backup/restore | PARTIAL | Fresh archive/import design selected; implementation absent. |
| A025 | Features/harness topology | PARTIAL | Eval and wave7 removed; remaining wave/query/proof and dormant-worker wiring need real caller cuts. |

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
| R032 | Canonical documentation/index reconciliation | [3a5d3032](https://github.com/UnknownAlienHuman/eliot-search/blob/3a5d30322466d40e78f501d324d5aa7a4cb17362/docs/audit/ANALYSIS_REGISTRY.md#r032-canonical-documentation-and-index-reconciliation) |
| R033 | Active package entry documentation | [95c5f65c](https://github.com/UnknownAlienHuman/eliot-search/blob/95c5f65ce8544cb14c08936993cb826f8cab4d9d/docs/audit/ANALYSIS_REGISTRY.md#r033-package-entry-documentation-recheck) |
| R034 | Baseline inventory label reconciliation | [this pass](#r034-baseline-inventory-label-reconciliation) |

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
- R032–R034 are documentation-only. Their verification is current-tree readback, link/anchor/status consistency
  and GitHub diff review; they add no Rust/runtime result.

<a id="r030-remove-disconnected-evaluation-authority"></a>

## R030. Remove disconnected evaluation authority

At source `fb18eb96`, `search-eval` had no external Rust caller; its daemon edge was Cargo-only. R030 removed the
41-file package, workspace/feature/dependency wiring and exact local lock entries. Future evaluation under #233
may be a small post-product tool over explicit finite result files; no facade, verdict authority or runtime gate
remains.

<a id="r031-adopt-maintained-markdown-offsetiter-correction"></a>

## R031. Adopt maintained Markdown OffsetIter correction

At source `2a04c1dd`, the materializer was the sole installed production consumer and still pinned 0.13.1.
R031 changed workspace/lock/parser/donor identity together to 0.13.5 and its published checksum. Run 40084893130
passed owner/consumer check and Clippy. #441 is closed as this code task; product-level parser cases remain
#215/#331 evidence.

<a id="r032-canonical-documentation-and-index-reconciliation"></a>

## R032. Canonical documentation and index reconciliation

At source `da382b66`, canonical architecture/product behavior was broadly consistent but entry pages lacked one
reading/delivery sequence and several status/anchor claims were stale. R032 rebuilt canonical navigation,
shortened live registers, preserved immutable history and synchronized #352/PR/#189. No product code changed.

<a id="r033-package-entry-documentation-recheck"></a>

## R033. Active package entry documentation recheck

At source `3a5d3032`, daemon README froze status at old head `5af8cde5`, CLI README claimed unexecuted one-shot
routes worked, and model-worker docs disagreed on current source size while its Rust still emitted retired P15/G6
wording. R033 corrected six Markdown files, recording route presence separately from execution and the exact
1,047-byte refusal stub. Rust remained unchanged.

<a id="r034-baseline-inventory-label-reconciliation"></a>

## R034. Baseline inventory label reconciliation

**Source:** `95c5f65ce8544cb14c08936993cb826f8cab4d9d`, 2026-10-24.

**Inspected:** current package entry docs and historical measurements for `search-contracts`, `search-domain`,
`search-ports`, source family, runtime family, Qdrant supervisor and optional encrypted-storage disposition;
R032/R033 indexes and registry; baseline audit/member disposition. No private #598 source was read.

**Finding:** several documents correctly retained useful gross measurements and caller-disposition evidence but
called them “current cost”, “current overhead” or “public main”. After extensive PR #610 deletions these figures
cannot describe the current branch. A historical caller census likewise cannot by itself prove a current package
has no remaining consumer.

**Decision/change:** bind those measurements explicitly to original baseline `c8e38b6`, label them historical
gross inventory, state that they are neither current recounts nor deletion targets, and require exact current
caller inspection at each code cut. Preserve the selected owners/removal boundaries and #598 protection. Update
docs navigation and this registry; no source behavior or product requirement changes.

**Publication follow-up completed:** PR #610, #352, #214, #334 and #266 were synchronized. Issue #266's title no
longer presents the historical 553 KiB measurement as a current task identity. Product implementation and all
runtime evidence above remain outstanding.
