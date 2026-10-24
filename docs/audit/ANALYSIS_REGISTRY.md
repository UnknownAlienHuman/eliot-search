# Analysis registry

Continuation memory for the current refactor. It records inspected evidence, decisions, published changes and
unrun work. It is not architecture, a permission system or a reason to delay known fixes.

## Resume after interruption

**Checkpoint:** R035, 2026-10-24 — active issue handoff contradictions reconciled.  
**Branch:** `codex/refactor-preparation-20261017`, [PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610).  
**Inspected head:** `877ac38ddbd9aae4709eeb4661d711ab68b6feae` (R034 handoff follow-up).  
**Original source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`.

R035 corrects active issue bodies which still treated historical source sizes/caller searches as permanent
current facts or described already removed packages as pending work. It changes issue documentation plus this
registry/index only; no Rust, Cargo, workflow or runtime behavior changes.

The persistent source → retained evidence → Qdrant → CLI application remains unconnected. Read
[docs/README](../README.md), then the affected owner recipe and exact current callers. Do not repeat R027–R035
as another repository-wide audit.

**Next implementation:** #333 typed settings and real consumers, then #270/#330/#331 source/object/preparation
and #264 the connected product path. #441 remains closed as the Markdown dependency/identity cut; connected
parser/materialize/replay evidence remains #215/#331 work. Coordinate only actual #598 object/key overlap.

Current workspace has **49 explicit members**. This is inventory, not a target package count.

## Established decisions — do not rediscover

- Documentation authority is architecture → implementation → nearest `FUNCTIONS.md`; product docs own
  workflows, registry owns evidence/status, donor register owns upstream facts.
- One current-format application owns root, source/object, preparation, publication, query and local session.
- Config receipts do not apply values; typed settings must reach actual root/source/Qdrant/query consumers.
- Query/publication/rebuild composition still uses disconnected oracle/in-memory/supplied-proof paths. Feature
  names and constructors do not connect them.
- Exact, orientation, comparison, documents and Git remain product requirements.
- Historical source-size, trait, operation, manifest and caller counts are tied to their inspected baseline.
  Recheck exact current callers at the code cut; old counts are neither current facts nor deletion targets.
- `search-retention` Cargo/workspace/lock wiring was removed in R020. Its remaining `FUNCTIONS.md` is a
  documentation-only maintenance owner; do not recreate or remove the crate again.
- Dormant optional packages and workers are not capabilities. Delete them only after an exact current caller
  search; future MCP/model/document work uses the current product path, not their old ledgers/stubs.
- R031 pins `pulldown-cmark 0.13.5` and matching materializer identity; #441 is closed as that dependency task.
- #598 private object/key work remains protected; consume only its accepted public handoff.

## Delivery remaining

| Group | Current owners/issues | Missing product result |
|---|---|---|
| Invocation/root/settings/source/preparation | [implementation owners](../implementation/README.md#owners), #333/#266/#270/#330/#331/#271 | Validated settings reach owners; retained current head and recoverable preparation. |
| Identity/Qdrant/publication | #259/#329/#263/#261/#300/#303 | Real SDK effects, atomic evidence/view, restartable rebuild and finite cleanup. |
| Query/evidence/window | [query](../../crates/search-query/README.md), #272/#279/#291/#293/#275/#249 | Scoped retrieval/Exact, pinned retained evidence and one bounded live-session window. |
| Protocol/client/CLI | #280/#332/#264 | One serviced framed child session and useful default CLI result. |
| Maintenance/archive/import | #302/#244 | Stopped-root collection, complete archive and fresh-destination import. |
| Product depth | [product behavior](../product/AGENT_ANALYSIS_FRAMEWORK.md), #281/#227/#335 | Source-backed orientation/comparison/documents/Git behavior. |

Optional evaluation (#233), MCP (#219), managed Qdrant (#334), overlays (#336) and models follow a working
baseline and do not form additional product subsystems.

## Coverage ledger

`CHECKED` applies only to the named question. `PARTIAL` means known implementation remains. `PROTECTED`
excludes private work. `RUNTIME-ONLY` requires execution.

| ID | Area | Status | Current result / remaining |
|---|---|---|---|
| A001 | Documentation authority | CHECKED for R032–R035 scope | Canonical indexes, active entry docs, historical/current labels and issue handoffs are aligned. |
| A002 | Historical governance | CHECKED | Waves/packets/swarm/generated authority removed; Git preserves history. |
| A003 | Workflows/wrappers | CHECKED | Old gates/wrappers removed; Actions remain manual-only. |
| A004 | Workspace inventory | CHECKED for inventory | 49 members; current full workspace/binaries remain unrun. |
| A005 | Entrypoints/product | PARTIAL | CLI/daemon shadowing mapped; real application absent. |
| A006 | Root/config/control | PARTIAL | Typed settings/root/current state not implemented. |
| A007 | Sources/reconciliation | PARTIAL | Acquisition/currentness defects known; retained-source integration unfinished. |
| A008 | Private object/key work | PROTECTED | Consume accepted #598 handoff only. |
| A009 | Preparation/identity | PARTIAL | Markdown parser/identity updated; connected preparation remains. |
| A010 | Qdrant/donors | CHECKED for named static questions | Live integration/quality/scale unrun. |
| A011 | Publication/view/cleanup | PARTIAL | Real effects/current-view path absent. |
| A012 | Query/evidence/window | PARTIAL | Connected path absent. |
| A013 | Protocol/client/CLI | PARTIAL | Serviced current session absent. |
| A014 | Product depth | PARTIAL | Requirements preserved; implementations incomplete. |
| A015 | Tests/governance code | PARTIAL | Remaining wave/query/lexical fixtures need caller cutover. |
| A016 | Donors | PARTIAL | Markdown installed; other donors remain per concrete consumer. |
| A017 | Qualification tree | CHECKED | Removed as authority; runtime evidence remains ordinary work. |
| A018 | Package/issue docs | CHECKED for R035 set | Package baselines and active issue status are no longer presented as current execution facts. |
| A019 | Issues/PR handoff | CHECKED for R035 | #302/#535/#330/#291/#274/#219 corrected; synchronize #352/PR/#214 after this registry commit. |
| A020 | Runtime evidence | RUNTIME-ONLY | No current Windows/Qdrant/recovery/performance result. |
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
| R035 | Active issue handoff reconciliation | [this pass](#r035-active-issue-handoff-reconciliation) |

Supporting historical evidence: [distilled audit](ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md),
[baseline member disposition](WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md) and the
[superseded coverage pointer](AUDIT_COVERAGE_AND_DOCUMENTATION_MAP_2026-10-21.md).

## Execution evidence

- [Run 39872058276](https://github.com/UnknownAlienHuman/eliot-search/actions/runs/39872058276): Rust 1.98
  `search-qdrant-bridge --lib` at `7d92fb8b`; no consumers/runtime.
- [Run 40084893130](https://github.com/UnknownAlienHuman/eliot-search/actions/runs/40084893130): Rust 1.98 locked
  check/minimal Clippy for `search-materializer` + `search-unitizer` at R031.
- Current binaries, Windows, live Qdrant, parser replay, restart/recovery, relevance and scale remain unrun.
- R032–R035 are documentation-only and add no Rust/runtime result.

<a id="r030-remove-disconnected-evaluation-authority"></a>

## R030. Remove disconnected evaluation authority

`search-eval` had no external Rust caller; R030 removed the 41-file package and Cargo/lock wiring. Future #233
evaluation is a small post-product tool, not a runtime gate.

<a id="r031-adopt-maintained-markdown-offsetiter-correction"></a>

## R031. Adopt maintained Markdown OffsetIter correction

Pinned `pulldown-cmark 0.13.5` and matching materializer identity. Run 40084893130 passed owner/consumer check
and Clippy. #441 is closed as this dependency task.

<a id="r032-canonical-documentation-and-index-reconciliation"></a>

## R032. Canonical documentation and index reconciliation

Rebuilt canonical navigation/delivery sequence, shortened live registers and synchronized current handoff docs.
No product code changed.

<a id="r033-package-entry-documentation-recheck"></a>

## R033. Active package entry documentation recheck

Corrected daemon/CLI execution claims and exact model-worker stub disposition. Six Markdown files; no Rust.

<a id="r034-baseline-inventory-label-reconciliation"></a>

## R034. Baseline inventory label reconciliation

Bound package measurements/caller censuses to original baseline `c8e38b6`, required current caller checks and
synchronized PR/#352/#214/#334/#266. Ten Markdown files plus one registry follow-up; no product code.

<a id="r035-active-issue-handoff-reconciliation"></a>

## R035. Active issue handoff reconciliation

**Source:** `877ac38ddbd9aae4709eeb4661d711ab68b6feae`, 2026-10-24.

**Inspected:** active issue bodies surfaced by historical `public main`/size/caller terminology and the current
R020/R032–R034 records. No private #598 source was read.

**Findings and changes:**

- #302 still said the empty `search-retention` crate awaited a Cargo cut although R020 already removed its
  workspace/daemon/lock wiring; it now identifies `FUNCTIONS.md` as documentation-only and forbids repeating the
  deletion.
- #535 treated an old no-caller result as permanent current truth; it now requires an exact current caller search
  immediately before deleting `search-eliot-adapter`.
- #330 presented two ~40 KiB kernels as current public-main state; it now binds the duplication to baseline
  `c8e38b6` and defers exact deletion to the accepted #598 handoff/current caller check.
- #291 and #274 presented historical package sizes/callers as current; they now preserve the architectural
  finding while requiring current caller inspection at implementation.
- #219 presented the adapter measurement/caller result as current and its 2026-10-20 SDK checkpoint as an
  automatic pin; it now separates historical adapter evidence from a required implementation-time donor recheck.

**Decision:** active issue bodies are handoff documents and must distinguish inspected historical evidence,
current branch facts, selected target behavior and executed results. Do not keep re-auditing the whole tree;
perform exact caller and donor checks within the concrete code slice.

**Remaining:** synchronize #352, PR #610 and #214 with R035. Product implementation and all runtime evidence
remain outstanding.
