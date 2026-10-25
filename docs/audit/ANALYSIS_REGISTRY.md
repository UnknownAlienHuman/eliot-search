# Analysis registry

Continuation memory for the current refactor. It records inspected evidence, decisions, published changes and
unrun work. It is not architecture, a permission system or a reason to delay known fixes.

## Resume after interruption

**Checkpoint:** R044, 2026-10-24 — active implementation documentation semantically closed.  
**Branch:** `codex/refactor-preparation-20261017`, [PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610).  
**Inspected head:** `8768926106ce013b980f159095d94ae862ad3608` (R043).  
**Original source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`.

R044 finishes the active implementation-document pass across query/evidence/window, publication planning and
protocol/client/CLI handoffs. Query planner, comparator, subject resolver and handle docs now label sizes and
caller searches as revision-specific. One query owner resolves scope against a pinned view, dispatches real
handlers, verifies retained evidence and owns one process-local `QueryWindow`. Projection and client/CLI docs use
the same current-caller cut rule.

The canonical active documentation is now internally aligned for implementation. Historical snapshots, retired
architecture and audit records remain immutable evidence rather than current authority. This is documentation
closure, not product completion or runtime verification.

The persistent source → retained evidence → Qdrant → CLI application remains unconnected. Read
[docs/README](../README.md), then the affected owner recipe and exact current callers. Do not restart a general
documentation audit; implement the selected delivery sequence and update only affected owners/evidence.

**Next implementation:** #333 typed settings and real consumers, then #270/#330/#331/#271
source/object/preparation/reconciliation and #264 the connected product path. Optional document/MCP/regex/
watcher/model/overlay/managed-Qdrant/persistent-pipe work remains behind its prerequisite and implementation-time
consumer/donor selection. Coordinate only actual #598 overlap.

Current workspace has **49 explicit members**. This is inventory, not a target package count.

## Established decisions — do not rediscover

- Documentation authority is architecture → implementation → nearest `FUNCTIONS.md`; product docs own user
  workflows, this registry owns evidence/status, and the donor register owns upstream facts.
- Historical sizes, source-shape findings and caller searches are revision-specific evidence. Before deletion,
  merge or inlining, recheck current manifests, Rust/non-Rust callers, constructors/module inclusion, features,
  targets, tests and workflows and move every surviving consumer in the same cut.
- One current-format application owns root, source/object, preparation, publication, query and local session.
- Root ownership is one retained root handle, one real exclusive lock and small current-format metadata. No
  durable owner lease, epoch, succession or release receipt is authority.
- Concrete consumers own concrete secrets. Trusted child stdio needs no pairing secret; Qdrant owns its optional
  key; a restricted-client adapter owns its credential/session; an encrypted immutable-object profile owns its
  selected OS-protected key and codec. Plain storage has no secret.
- #598 private object/key work remains protected. `search-revision-crypto` survives only if the accepted public
  handoff directly consumes it; otherwise it is deleted after exact current caller inspection.
- Child stdio is the baseline local protocol. A persistent Windows pipe is optional and requires a real server,
  owner/ACL validation and a current consumer.
- Config receipts do not apply values; typed settings must reach actual constructors.
- Query has one owner: typed request + pinned `PublishedView` + non-widening scope + one absolute context + real
  handlers. No public plan/grant/capability/recipe authority is required.
- `Search` calls the real retrieval handler; `Exact` scans pinned inventory; evidence resolves through the pinned
  manifest and immutable reader; `Continue`/`Expand` use one process-local `QueryWindow`.
- Comparator and subject-resolution helpers consume real source-backed producers or remain small/inlined. Names,
  scores, digests and supplied receipts do not prove identity, conflict, equivalence or independent lineage.
- Result cards carry verified excerpts/locations. Expansion uses an emitted item locator inside the same window;
  no second handle store, token authority, whole-catalogue fence or arbitrary revision range survives.
- Publication owns one pending external-effect record, current view and finite cleanup rows. Projection planning
  uploads actual bounded data; hashes/manifests are not substitutes for effects. Stopped-root local-object
  maintenance remains separate.
- Literal Exact is baseline; optional regex cannot block it or reserve a speculative dependency.
- Reconciliation must work independently before optional watcher hints.
- `search-retention` is not a Cargo package to restore. Its documentation owns stopped-root local-object
  collection and archive/import; publication owns online Qdrant cleanup.
- An inspected donor is not installed. Exact version, features, MSRV, advisories and native artifact move with
  the real consuming implementation.
- `rmcp`, PDFium/`pdfium-render`, `regex-automata`, `notify`, high-level TOML/Serde and model runtimes are absent
  from the current lock unless a later implementation commit explicitly adds them.
- R031 pins `pulldown-cmark 0.13.5`; #441 is closed as that dependency task.

## Delivery remaining

| Group | Current owners/issues | Missing product result |
|---|---|---|
| Invocation/root/settings/source/preparation | [implementation owners](../implementation/README.md#owners), #333/#266/#270/#330/#331/#271 | Validated settings reach constructors; retained current head and recoverable reconciliation/preparation. |
| Identity/Qdrant/publication | #259/#329/#263/#261/#300/#303 | Real SDK effects, atomic evidence/view, restartable rebuild and finite cleanup. |
| Query/evidence/window | [query](../../crates/search-query/README.md), #272/#279/#291/#293/#275/#249 | Scoped retrieval and Exact, pinned retained evidence and one bounded live-session window. |
| Protocol/client/CLI | #280/#332/#264 | One serviced framed child session and useful default CLI result. |
| Maintenance/archive/import | #302/#244 | Stopped-root collection, complete archive and fresh-destination import. |
| Product depth | [product behavior](../product/AGENT_ANALYSIS_FRAMEWORK.md), #281/#227/#335 | Source-backed orientation/comparison/documents/Git behavior. |

Optional regex (#248), watcher (#239), evaluation (#233), MCP (#219), managed Qdrant (#334), overlays (#336),
models and persistent Windows pipes follow working prerequisites and do not form baseline subsystems.

## Coverage ledger

| ID | Area | Status | Current result / remaining |
|---|---|---|---|
| A001 | Documentation authority | CHECKED for active implementation docs | Canonical indexes, owners, handoffs and currentness rules aligned through R044. |
| A002 | Historical governance | CHECKED | Removed as current authority; Git preserves history. |
| A003 | Workflows/wrappers | CHECKED | Actions remain manual-only. |
| A004 | Workspace inventory | CHECKED for inventory | 49 members; current full workspace/binaries remain unrun. |
| A005 | Entrypoints/product | PARTIAL | Shadowing mapped; real application absent. |
| A006 | Root/config/control | PARTIAL | Target precise; typed settings/root implementation remains. |
| A007 | Sources/reconciliation | PARTIAL | Sync entrypoint unavailable; watcher correctly deferred. |
| A008 | Private object/key work | PROTECTED | Consume accepted #598 handoff only. |
| A009 | Preparation/identity | PARTIAL | Documentation selected; connected preparation remains. |
| A010 | Qdrant/donors | CHECKED for named static questions | Live integration/quality/scale unrun. |
| A011 | Publication/view/cleanup | PARTIAL | Owners/algorithms documented; real effects/current-view path absent. |
| A012 | Query/evidence/window | PARTIAL | Owners/cutover documented; connected path absent. |
| A013 | Protocol/client/CLI | PARTIAL | One-session target documented; serviced session absent. |
| A014 | Product depth | PARTIAL | Requirements preserved; implementations incomplete. |
| A015 | Tests/governance code | PARTIAL | Remaining fixtures need caller cutover. |
| A016 | Donors | PARTIAL | Installed versus candidate state explicit; current pins remain per consumer. |
| A017 | Qualification tree | CHECKED | Removed as authority. |
| A018 | Package/issue docs | CHECKED for active implementation layer | Runtime, source/prep, query, projection and client/CLI currentness reconciled. |
| A019 | Issues/PR handoff | PENDING PUBLICATION SYNC | Update #332/#352 and PR #610 to exact R044 SHA before ref advancement. |
| A020 | Runtime evidence | RUNTIME-ONLY | No current Windows/Qdrant/reconciliation/regex/PDF/MCP/overlay/performance result. |
| A021 | Compiler baseline | PARTIAL | R031 covers materializer+unitizer only. |
| A022 | Runtime identity/qualification | PARTIAL | Documentation corrected; obsolete code/fixtures remain. |
| A023 | GC/storage diagnostics | PARTIAL | Ownership corrected; implementation unfinished. |
| A024 | Backup/restore | PARTIAL | Archive/import implementation absent. |
| A025 | Features/harness topology | PARTIAL | Exact current caller cuts occur inside implementation commits. |

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
| R036 | Completed removal/PDF handoff | [b6e70f71](https://github.com/UnknownAlienHuman/eliot-search/blob/b6e70f716d855a435d9a0bef8de5086de0a03867/docs/audit/ANALYSIS_REGISTRY.md#r036-completed-removal-and-pdf-donor-handoff-reconciliation) |
| R037 | Watcher prerequisite/donor | [e1b66223](https://github.com/UnknownAlienHuman/eliot-search/blob/e1b66223ff255349b18135fb810a13afaa496ed6/docs/audit/ANALYSIS_REGISTRY.md#r037-watcher-prerequisite-and-donor-state) |
| R038 | Optional regex dependency | [d3d81ca0](https://github.com/UnknownAlienHuman/eliot-search/blob/d3d81ca0fc47a68c03235f21b276af0f46fb64f3/docs/audit/ANALYSIS_REGISTRY.md#r038-optional-regex-dependency-state) |
| R039 | Optional MCP SDK state | [0a9aa731](https://github.com/UnknownAlienHuman/eliot-search/blob/0a9aa73186eb7b7fb0c25c35d9ba947c2ba8fd4f/docs/audit/ANALYSIS_REGISTRY.md#r039-optional-mcp-sdk-state) |
| R040 | Document-worker candidate state | [8a63e7ad](https://github.com/UnknownAlienHuman/eliot-search/blob/8a63e7ad770ea4d57f506c54f07716bb06203111/docs/audit/ANALYSIS_REGISTRY.md#r040-document-worker-candidate-state) |
| R041 | Adapter/model-worker currentness | [6de4d2b3](https://github.com/UnknownAlienHuman/eliot-search/blob/6de4d2b3788b94714b59351cdb5dc5d0f0d13fd5/docs/audit/ANALYSIS_REGISTRY.md#r041-revision-currentness-and-donor-checkpoint) |
| R042 | Package currentness/maintenance ownership | [ca6c7ed4](https://github.com/UnknownAlienHuman/eliot-search/blob/ca6c7ed4b00973586ac0c5f989b9d7b532946cac/docs/audit/ANALYSIS_REGISTRY.md#r042-package-currentness-and-maintenance-ownership) |
| R043 | Runtime ownership/currentness | [87689261](https://github.com/UnknownAlienHuman/eliot-search/blob/8768926106ce013b980f159095d94ae862ad3608/docs/audit/ANALYSIS_REGISTRY.md#r043-runtime-ownership-and-currentness) |
| R044 | Query/publication/protocol currentness and documentation closure | [this pass](#r044-query-publication-protocol-currentness-and-documentation-closure) |

## Execution evidence

- Run 39872058276 covers `search-qdrant-bridge --lib` at old commit `7d92fb8b` only.
- Run 40084893130 covers locked check/minimal Clippy for materializer + unitizer at R031 only.
- Current binaries, Windows, live Qdrant, reconciliation/watcher, regex, PDF, MCP, model/overlay integration,
  parser replay, recovery, relevance and scale remain unrun.
- R032–R044 are documentation-only and add no Rust/runtime result.

<a id="r044-query-publication-protocol-currentness-and-documentation-closure"></a>

## R044. Query/publication/protocol currentness and documentation closure

**Source:** `8768926106ce013b980f159095d94ae862ad3608`, 2026-10-24.

**Inspected:** query family index; access, evidence validator, Exact, continuation, handles, planner, executor,
projector, comparator and subject resolver instructions/maps; Qdrant family index, epoch pins, reclaimer,
projection planner and publication instructions; provider protocol/client README/AGENTS; CLI README/AGENTS;
issues #281/#332 and current documentation index. No private #598 source was read.

**Findings:** query-planner README/FUNCTIONS called a 104,042-byte/test-only audit “verified current”; comparator
and subject resolver called package-test callers current; handles called two historical store sizes/current daemon
reachability current; continuation deletion and projection-planner disconnection lacked the exact current caller
cut rule. Provider-client and CLI status used current wording for inspected legacy routes. The core protocol,
publication, access, evidence and Exact instructions were otherwise consistent with one pinned-view path.

**Decision/change:** label all those measurements/caller findings revision-specific; require exact current
manifest/Rust/non-Rust/constructor/module/feature/target/test/workflow searches inside code cuts. Establish one
direct query owner, one evidence path and one `QueryWindow`; remove public planner authority, supplied-candidate
proof planes and the second handle store after callers move. Projection planning must carry actual bounded data to
publication. Client/CLI use one serviced framed owned-child session. Canonical active docs are now aligned; retired
and historical records stay evidence only.

**Unrun/remaining:** no current caller search, code deletion, compiler, Qdrant, transport or runtime execution is
performed in this documentation slice. Product implementation remains. Future documentation changes should
follow concrete implementation cuts or new verified evidence, not restart a general audit.
