# Analysis registry

Audit continuation memory, not another specification or permission system. A finding, corrected instruction,
production change and executed check are different facts. Architecture and owner recipes direct implementation.

## Resume after interruption

**Checkpoint:** R026, 2026-10-23 — recovery, indexing and documentation-status reconciliation only.  
**Branch:** `codex/refactor-preparation-20261017`, [PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610).  
**Inspected checkpoint:** `be84c8abb395e2b96426cf7b1bb87a5cae4d062d`.  
**Last production-source analysis:** R025, published as `be84c8ab`, inspected `2d33dbde`.  
**Original source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`.

The interrupted session had already published R025. R023 (`676135f4`) and R024 (`2d33dbde`) are also in the branch;
the old downloadable R023 patch is not pending work. Do not apply it again over newer owner documents.

The project is in audit/refactor preparation, not a completed rewrite. The indexed source → retained evidence →
CLI application is not connected; the current binary build is not established by the historical library checks.
Deleting obsolete code or correcting help does not deliver its replacement.

To resume, read current PR head, inspect any intervening commits, then load only the chosen workset below and
its owner recipe. Current branch links are for navigation; immutable checkpoint links are evidence. Default-branch
code search finds paths but cannot prove caller absence on this branch. Use one manager-owned worktree.

**Next source question: [S1](#s1-consumers-and-removal-boundaries).** Finish outside callers and shared effects of
the retired native/provider family, starting at `native_binding/{opening,registration,startup,journal_open}.rs`.
Do not reread R025's handshake/work/local-stream files as new work, or repair a whole obsolete platform merely
to remove it. Their exact readset and unresolved qualifications remain in the [R025 record](https://github.com/UnknownAlienHuman/eliot-search/blob/be84c8abb395e2b96426cf7b1bb87a5cae4d062d/docs/audit/ANALYSIS_REGISTRY.md).

## Counted remainder

**25 areas: 6 CHECKED in their named scope, 17 PARTIAL, 1 PROTECTED, 1 RUNTIME-ONLY.** Checked IDs:
A002/A003/A004/A010/A017/A018. These are not a completion percentage. PARTIAL can mean known code corrections
remain; it does not reset inspected files to unread. No complete file-level denominator or remaining-defect
count exists. The five static worksets and six delivery groups below are not estimates of commits or duration.

The root manifest at `be84c8ab` explicitly lists **50 workspace members**. The 52-row disposition document describes
the original baseline, not today's workspace or a target size; `xtask` and `search-retention` are no longer members.
This is a manifest recount, not Cargo dependency resolution.

## Remaining static audit

All five worksets remain open. A workset is closed by a recorded answer and correction of conflicting instructions,
not by finishing the entire product. Already-established defects awaiting implementation belong to delivery work.
Paths below are read entrypoints, not instructions to create new packages or repair every historical primitive.

### S1. Consumers and removal boundaries

Areas A005/A006/A012/A013/A025. Owner: [daemon FUNCTIONS](../../bins/eliot-searchd/FUNCTIONS.md), with
[CLI](../../bins/eliot-search/FUNCTIONS.md) and [client](../../crates/search-provider-client/FUNCTIONS.md).

Trace current-branch wave1–wave6, shared sealed/migration and outside native/provider/query callers from actual
entrypoint to constructor, effect and output. Separate exclusive obsolete code from live source/status helpers,
OS-secret effects and protected object/key work. Output: named removal/receiving-owner decisions with callers;
no reconstructed registration platform, blanket directory deletion or caller-absence claim from main search.

Immediate follow-up paths under the [native_binding directory](../../bins/eliot-searchd/src/access_composition/native_binding/):

```text
opening.rs; opening/{connections,credential,tcp}.rs
registration.rs; registration/{finalization,intent,provisioning,readback}.rs
startup.rs; journal_open.rs; codec.rs only where a surviving caller requires it
```

R026 confirmed these paths from current GitHub directory/tree metadata, not their code semantics. `process.rs`
and the parent binding/listener/security/live-authority files were read in R025; use that evidence first.
The two subtree listings were complete (`truncated=false`). This is not a whole-repository caller inventory.

### S2. Surviving documentation and issue consistency

Areas A001/A019. Start with [documentation index](../README.md), the nearest README/AGENTS/FUNCTIONS and the
corresponding issue. Compare useful historical requirements against current owners; one algorithm has one home.
Output: corrected owner rule and links, or an explicit retained/deferred/unsupported behavior. No new master spec.

R026 fixes stale audit-closure navigation in this directory, `docs/README.md` and #352; it does not reconcile all
package instructions or all issues. The old closure map is now a pointer to current evidence and its archived text.

### S3. Configuration and transformation identity

Areas A009/A022 and the configuration part of A006. Start with
[lexical](../../crates/search-lexical/FUNCTIONS.md),
[materializer](../../crates/search-prep/search-materializer/FUNCTIONS.md), and
[config](../../crates/search-config/FUNCTIONS.md), then their actual consumers.

R009/R011 already establish fixture-backed activation and test/limit identity coupling. Finish the caller map:
which values change transformed output, guard current installation, limit execution, or permit retry? Output:
concrete caller changes/deletions preserving useful tests as tests, not another profile/qualification registry.

### S4. Mixed migration fixtures

Area A015 with S1's shared effects. Start at the two current Windows-only suites:

- [control_migration_bootstrap_boundary.rs](../../bins/eliot-searchd/tests/control_migration_bootstrap_boundary.rs);
- [control_migration_inventory_owner_boundary.rs](../../bins/eliot-searchd/tests/control_migration_inventory_owner_boundary.rs).

Extract actual filesystem/no-clobber/drift/alias/incomplete-state cases from source-text assertions and obsolete
phases, using [TEST_CUTOVER](../implementation/TEST_CUTOVER.md). Trace production dependencies before removal.
R026 checked only lines 1–30 to confirm current paths/scope; it neither reread entire suites nor executed them.
Output: named DELETE / MOVE CAUSAL CASE / RETAIN decisions tied to current owners, not a new test framework.

### S5. Product depth and selected donors

Areas A014/A016. Start with [product behavior](../product/AGENT_ANALYSIS_FRAMEWORK.md),
[query overview](../../crates/search-query/README.md), [exact](../../crates/search-query/search-exact/FUNCTIONS.md),
and the [donor index](DONOR_VERIFICATION_REGISTER_2026-10-09.md). Later donor evidence is also indexed below.

For Exact, navigation/ambiguity/comparison, documents, selected models/adapters and MCP, identify a real producer,
consumer and small implementation seam, or explicitly record the unsupported/deferred boundary. Preserve useful
old requirements; deleting scaffolds is not delivery. Inspect donors only to answer a selected question, with
version/source and cost limitations. Output: owner decisions, not empty workers/registries or every SDK method.
Quality and scale require the implemented application and execution, not more research prose.

## Delivery remaining, separate from static audit

| Group | Current owner entrypoint | What is still not delivered |
|---|---|---|
| Source/object/current state/preparation | [implementation owners](../implementation/README.md#owners) | Connected retained bytes, current head and recoverable preparation with recorded source/budget/empty/stale-work fixes. |
| Qdrant/publication | [publication](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) | Real immutable retrieval/view publication, pending effects, rebuild and cleanup through one runtime. |
| Query/evidence/windows | [query](../../crates/search-query/README.md) | Direct scoped retrieval, actual retained evidence and one bounded pinned window. |
| Session/client/CLI | [protocol](../../crates/search-provider-protocol/FUNCTIONS.md) | Connected framing/control/cancellation and owned-child path replacing the legacy clients. |
| Maintenance/archive/import | [maintenance notes](../../crates/search-runtime/search-retention/FUNCTIONS.md) | Current-format stopped-root collection, complete archive and fresh-destination import; this path is notes, not a crate to recreate. |
| Selected product depth | [product behavior](../product/AGENT_ANALYSIS_FRAMEWORK.md) | Actual exact/navigation/comparison/document and selected adapter behavior, not just constructors or stubs. |

Minimal pinned compiler/Clippy follows production changes; broader causal/platform/recovery/relevance/scale checks
follow their implementation. #598 protects actual unpublished object/key overlap only. Preserve user data and
shared storage primitives; its retired six-domain/purge/compatibility programme does not block unrelated work.

## Coverage ledger

CHECKED is limited to the stated question. PARTIAL records named inspection or implementation work. PROTECTED
excludes unpublished work; RUNTIME-ONLY requires execution. No existing area ID is removed or silently upgraded.

| ID | Area | Status | Established result / current state | Remaining |
|---|---|---|---|---|
| A001 | Documentation authority | PARTIAL | R026 removes contradictory closure pointers and provides one continuation index. Prior owner corrections remain in their pass records. | S2; other local instructions and useful historical requirements. |
| A002 | Historical governance data | CHECKED | Swarm/maps/packets/generated status removed; Git preserves history. | Code remnants tracked separately. |
| A003 | Workflows/wrappers | CHECKED | Old gates/wrappers removed; Actions manual-only. | Recheck when changed, not a new gate. |
| A004 | Workspace inventory | CHECKED for inventory | R026 recounts 50 explicit members at be84c8ab; the 52-row plan is historical. | Actual Cargo resolution and remaining package cuts. |
| A005 | Entrypoints/product | PARTIAL | Help/readiness corrected; root forwarding and native constructors do not form indexed Search. | S1 and actual default CLI/daemon integration. |
| A006 | Root/config/control | PARTIAL | One root guard/typed state selected; R025 records intentional forgetting of legacy process ownership. | S1/S3 and current-state/platform implementation. |
| A007 | Sources/reconciliation | PARTIAL | R010 defects and live provider currentness consumer recorded. | Source/object/current-row integration and published freshness. |
| A008 | Private object/key work | PROTECTED | Public contract reviewed; retired programme not inherited. | Accepted handoff; no unpublished branch/worktree changes. |
| A009 | Preparation/identity | PARTIAL | Test/limit coupling, budget/empty/stale/ABA distinctions established. | S3 and implementation through actual consumers. |
| A010 | Qdrant/donors | CHECKED for named static questions | Gate removed; BM25/tokenizer/IDF/readback/late-write limitations recorded. | Real integration and relevance/scale/late-write execution. |
| A011 | Publication/view/cleanup | PARTIAL | Two-sided reconciliation, immutable retrieval and durable captured inputs selected. | Crash/view/rebuild/cleanup code, not rediscovery of old findings. |
| A012 | Query/evidence/window | PARTIAL | R025 traces population validation and shared locks through backend/output; pinned-view replacement selected. | S1 and actual query/evidence/window integration. |
| A013 | Protocol/client/CLI | PARTIAL | R023–R025 named helper/authority readsets preserved. R026 indexes remaining native paths only. | S1 and serviced session/platform execution. |
| A014 | Product depth | PARTIAL | Useful exact/navigation/comparison/documents and selected adapters remain requirements. | S5 and concrete implementation/quality. |
| A015 | Tests/governance code | PARTIAL | Structural/disconnected proofs removed; useful causal cases retained. R026 locates two mixed suites. | S4 and residual structural consumers. |
| A016 | Donors | PARTIAL | redb/Qdrant/ignore/Tokio findings preserved with version/source limitations. | S5; no new donor inspection in R026. |
| A017 | Qualification tree | CHECKED | Archived; policy/blocker notes remain. | Runtime ceremony under A022. |
| A018 | Auxiliary documents | CHECKED for filename/disposition | Duplicate supplements removed; retention path is documentation only. | Semantic consistency under A001. |
| A019 | Issues/PR handoff | PARTIAL | R026 reconciles #352 and PR with the registry; no whole-queue claim. | S2 for other affected owners/issues. |
| A020 | Runtime evidence | RUNTIME-ONLY | Static findings and deleted fixtures do not establish platform/recovery/performance. | Exact-SHA relevant runs after implementation. |
| A021 | Compiler baseline | PARTIAL | Historical bridge-lib run 39872058276 covers 7d92fb8b only. No R026 compilation. | Actual binary/current-consumer checks and locked resolution. |
| A022 | Runtime qualification/identity | PARTIAL | Qdrant activation gone; lexical/materializer fixture authority remains. | S3 and real caller cleanup. |
| A023 | GC/storage diagnostics | PARTIAL | Generic platform gone; current collector and hot-path inventory removal unfinished. | Actual implementation, not rediscovery. |
| A024 | Backup/restore | PARTIAL | Fresh archive/destination design recorded; generic restore removed. | Actual archive/import and interrupted/protected cases. |
| A025 | Features/harness topology | PARTIAL | Wave7/full-baseline removed; named native families read, remaining paths indexed. | S1 before broader Cargo pruning; no whole-tree caller claim. |

## Evidence index by pass and topic

Each link opens the full checkpoint, including inspected paths and limits. Earlier records are evidence, not
current implementation instructions. Use the current owner recipe when a later pass supersedes a design.

| Pass | Topics / useful lookup | Immutable record |
|---|---|---|
| R001–R012 | Governance/packages; R009 lexical; R010 source observation; R011 preparation/publication; R012 evidence/windows | [44d7416c](https://github.com/UnknownAlienHuman/eliot-search/blob/44d7416c46554456f16ccd2b3943657b755304ee/docs/audit/ANALYSIS_REGISTRY.md) |
| R013 | Client display, active cancellation, framing and child cleanup | [2b37f5ef](https://github.com/UnknownAlienHuman/eliot-search/blob/2b37f5ef1e8cc755bbcd8c0e1f7efc840eff8cf3/docs/audit/ANALYSIS_REGISTRY.md) |
| R014 | Query-time storage inventories, local GC, archive/restore isolation | [8cf331a4](https://github.com/UnknownAlienHuman/eliot-search/blob/8cf331a44decded2c84c7f579dcbf8434e040dc7/docs/audit/ANALYSIS_REGISTRY.md) |
| R015 | Structural/lifecycle fixtures; preserve causal behavior | [bf2bde29](https://github.com/UnknownAlienHuman/eliot-search/blob/bf2bde299b3f009527397571bd9baeea51fde840/docs/audit/ANALYSIS_REGISTRY.md) |
| R016 | Standalone snapshot/sealed applications | [53ab0ea3](https://github.com/UnknownAlienHuman/eliot-search/blob/53ab0ea33f8d405f72e984881b5aea0db69e7c3b/docs/audit/ANALYSIS_REGISTRY.md) |
| R017 | Generic lifecycle/restore removal | [15145667](https://github.com/UnknownAlienHuman/eliot-search/blob/151456672ea4ec8773ed65c5017cb63989912d30/docs/audit/ANALYSIS_REGISTRY.md) |
| R018 | Disconnected query/provider/rebuild process proofs | [174f3ec7](https://github.com/UnknownAlienHuman/eliot-search/blob/174f3ec758cb0ec5c38b780cd3de913be9120fdc/docs/audit/ANALYSIS_REGISTRY.md) |
| R019 | Daemon TCP/token endpoint and proxy removal | [156528ad](https://github.com/UnknownAlienHuman/eliot-search/blob/156528ad0a2ab64599d9cef01d3eeea2b6f11040/docs/audit/ANALYSIS_REGISTRY.md) |
| R020 | Retention Cargo/lock shell, wave7/full-baseline, evidence limits | [e6c7bb90](https://github.com/UnknownAlienHuman/eliot-search/blob/e6c7bb902c9aa91509ceeccae996c610f2b8f0cd/docs/audit/ANALYSIS_REGISTRY.md) |
| R021 | Feature roots, truthful diagnostics, live currentness callers | [ff59f930](https://github.com/UnknownAlienHuman/eliot-search/blob/ff59f930c5b160b1ed36684538be3dca1bdd6970/docs/audit/ANALYSIS_REGISTRY.md) |
| R022 | Replay capacity, cancellation, subprocess capture and clocks | [305ccab1](https://github.com/UnknownAlienHuman/eliot-search/blob/305ccab141774dba088e38dacc006fc90fcfb0a2/docs/audit/ANALYSIS_REGISTRY.md) |
| R023 | Native/legacy constructors, lost installation anchor, receipt scope | [676135f4](https://github.com/UnknownAlienHuman/eliot-search/blob/676135f49c5373b27edfd6429717464e53202974/docs/audit/ANALYSIS_REGISTRY.md) |
| R024 | Idle/frame clocks, terminal races, Tokio 1.53.1, CLI help | [2d33dbde](https://github.com/UnknownAlienHuman/eliot-search/blob/2d33dbdeefce0eb0c564f013a8e66032e73f6693/docs/audit/ANALYSIS_REGISTRY.md) |
| R025 | Native authority lock cost, forgotten ownership, Interrupted accept, admission order | [be84c8ab](https://github.com/UnknownAlienHuman/eliot-search/blob/be84c8abb395e2b96426cf7b1bb87a5cae4d062d/docs/audit/ANALYSIS_REGISTRY.md) |
| R026 | Recovery/status/navigation reconciliation; directory-only next-read index | [this pass](#r026-recovery-and-index-reconciliation) |

Earlier source/donor material: [distilled audit](ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md),
[donor evidence](DONOR_VERIFICATION_REGISTER_2026-10-09.md), and
[baseline package dispositions](WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md).
The [qualification archive](https://github.com/UnknownAlienHuman/eliot-search/tree/2f9c9720f3e14799a0f8dfefa00077d2fa6a9887/qualification)
is historical, not current readiness. Later Tokio/source observations live in R024/R025, not only the older donor file.

## Execution evidence

Last previously recorded passing run: [39872058276](https://github.com/UnknownAlienHuman/eliot-search/actions/runs/39872058276),
`7d92fb8b`, Ubuntu/Rust 1.98.0, `search-qdrant-bridge --lib` check and strict Clippy. R026 does not reverify its logs
or extend its scope. Earlier broad daemon compilation failed. R020 lock pruning is manually reviewed, not a
Cargo-regenerated result. No current CLI/daemon, Windows, live Qdrant or end-to-end success follows.

R026 changes Markdown only; no compiler/test/runtime run. Tool lookup in this session found no cargo/rustc.
A direct archive request failed DNS; exact repository reads and publication use GitHub APIs. No full checkout,
resolved dependency graph, exhaustive link crawl, whole-tree caller scan or subagent execution is claimed.

## R026. Recovery and index reconciliation

Inspected repository checkpoint `be84c8abb395e2b96426cf7b1bb87a5cae4d062d` on 2026-10-23.

**Content inspected:** root AGENTS; architecture; README lines 1–48; implementation lines 1–60;
root Cargo.toml lines 1–95 including the complete members list; docs/README; audit README, coverage map,
workspace disposition and complete analysis registry; donor register lines 1–65; the two S4 fixture headers
(lines 1–30 each); PR #610, issue #352 and R025 commit metadata/diff.

**Metadata only:** docs/audit directory; native_binding directory and complete opening/registration subtree
listings. Paths discovered there are indexed in S1, not marked semantically checked. No new production Rust
path was deeply audited in this recovery pass, and the architecture/implementation rules were not redesigned.

**Reconciled defects in the audit material:** the old coverage map still claimed static closure and all 288 PR
paths/active issues reconciled; audit README and docs/README promoted that claim; #352 pinned current authority
to an older commit and linked the same closure map. Those conflict with the explicit five-workset remainder.
The old map's full text remains at be84c8ab; its current path redirects to this ledger and actual owner recipes.
#352/PR use current-branch entrypoints instead of another copied algorithm/status queue.

**Persistence:** this registry retains all 25 area IDs and every prior immutable checkpoint, adding a topic
index, S1–S5 read questions and a separate delivery list. Resume from the published PR head, not chat memory or
a stale local patch. For each further pass append exact inspected scope, finding/disposition, changed paths and
execution result; retain a permalink before condensing any older detailed record. Listing a file is not reading
it, documentation is not implementation, and an absent check is not a passing check.

This completes the recovery/indexing task only. S1–S5, known implementation defects and runtime gaps stay open.
