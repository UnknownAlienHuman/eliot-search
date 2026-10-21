# ELIOT Search — audit coverage and documentation disposition

**Audited public baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Closure status:** **STATIC AUDIT COMPLETE FOR REFACTOR PREPARATION**  
**Product/runtime qualification:** false.

This is the single map of what the audit established. “Complete” means that the current public architecture,
principal implementations, real callers, competing owners, active implementation queue and documentation
authority were traced deeply enough to define the refactor. It does **not** mean every Rust line or fixture
was read and does not claim native/runtime/live-Qdrant/recovery/performance execution.

## 1. Status vocabulary

| Status | Meaning |
|---|---|
| **SOURCE-TRACED** | Principal implementation, entrypoint, real callers and competing owners were inspected deeply enough to define the keep/merge/delete boundary. |
| **INVENTORIED** | The family was measured/classified; individual files are reviewed while their owning cutover is implemented. |
| **OPTIONAL** | It cannot block the baseline and may return only through a selected concrete consumer/profile. |
| **NOT EXECUTED** | No native/runtime/live-Qdrant/performance/recovery claim exists. |
| **OUTSIDE AUDIT** | Deliberately not inspected because another active manager owns unpublished work. |

The audit does not use a fake completion percentage. Its closure criterion is a concrete owner/disposition for
every workspace member, one implementation graph, a reconciled active issue queue and unambiguous document
authority.

## 2. Coverage and result

| Area | Status | What was established | Current map |
|---|---|---|---|
| Architecture, ADRs and old requirements | SOURCE-TRACED | Architecture 8.4/waves, compatibility, migration, security, qualification and process requirements were compared with the actual local product. The short manifesto supersedes them. | [`docs/architecture/README.md`](../architecture/README.md) |
| Workspace, packages, features and entrypoints | SOURCE-TRACED | All 52 Cargo members received **keep/rewrite**, **merge/inline**, **delete**, **optional** or **#598-conditional** disposition. Shadow applications, harness binaries and feature ladders were mapped. | [`WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md`](WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md) |
| Root ownership and daemon composition | SOURCE-TRACED | Real lock/handle authority was separated from epochs, leases, owner slots, migration custody and receipt history. | [`search-runtime-owner/FUNCTIONS.md`](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md) |
| Configuration and readiness | SOURCE-TRACED | Custom parser/tree/registry/fingerprint/diff/action/receipt stack and startup refusal were traced; one typed settings value replaces it. | [`search-config/FUNCTIONS.md`](../../crates/search-config/FUNCTIONS.md) |
| Source admission/read/registry/history/reconcile/Git | SOURCE-TRACED | Dropped acquired body, registry replay/original inputs, receipt/provenance archives, sync refusal and watcher/inventory gaps were traced to real callers. | [`crates/search-source/README.md`](../../crates/search-source/README.md) |
| Immutable objects and storage protection | SOURCE-TRACED, #598-CONDITIONAL | Duplicate publication kernels, staging leaks and Windows durability gap were traced. Final crypto/Windows package disposition depends only on the accepted #598 handoff. | [`search-revision-store/FUNCTIONS.md`](../../crates/search-source/search-revision-store/FUNCTIONS.md) |
| Materialization and unitization | SOURCE-TRACED | Adjacent-stage reparsing, duplicate artifact writer, copy/serialization waste, ExactBytes restrictions and revision/offset identity amplification were traced. | [`search-materializer/FUNCTIONS.md`](../../crates/search-prep/search-materializer/FUNCTIONS.md) |
| Retrieval point identity and projection | SOURCE-TRACED | Legacy identity and revision-bound S11 v1 were both rejected; stable retrieval identity was separated from versioned evidence binding. Whole plans/manifests become bounded pages. | [`search-point-identity/README.md`](../../crates/search-index-qdrant/search-point-identity/README.md) |
| Qdrant supervisor/bridge/query | SOURCE-TRACED | Real SDK code, disconnected daemon callers, qualification/auth ceremony, process-local receipt ledger, schema/profile defects, native BM25/IDF/fusion and exact mutation semantics were traced. | [`search-qdrant-bridge/FUNCTIONS.md`](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) |
| Publication, view lifetime, cleanup and GC | SOURCE-TRACED | Three publication machines, incomplete durable recovery, raw pins/watermarks/reclaimer/retention overlap and correct multi-view cleanup threshold were traced. | [`search-publication/FUNCTIONS.md`](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) |
| Query, exact scan and evidence | SOURCE-TRACED | Recipe/capability/plan/grant stack, scheduler/tickets/count, fusion identity, readback-proof defect, projector reorder and exact-proof platform were traced. | [`crates/search-query/README.md`](../../crates/search-query/README.md) |
| Handles, continuation and output | SOURCE-TRACED | Four overlapping stores, whole-catalog fences, prefix rescans, arbitrary revision expansion and useful commit-after-complete-output behavior were traced. | [`search-continuation/FUNCTIONS.md`](../../crates/search-query/search-continuation/FUNCTIONS.md) |
| Protocol, clients and CLI | SOURCE-TRACED | Pairing/MAC/grant/manual JSON stacks, typed client, daemon provider platform, separate line protocol, tab/hex translation and child-stdio ownership were traced. | [`search-provider-protocol/FUNCTIONS.md`](../../crates/search-provider-protocol/FUNCTIONS.md) |
| Shared contracts, domain and ports | SOURCE-TRACED | Planned recipe/proof/lifecycle catalogue, domain ownership drift and 23 global traits/80 operations were traced to actual callers. Concrete owners replace the global port OS. | [`search-contracts/README.md`](../../crates/search-contracts/README.md) |
| Tests and harnesses | INVENTORIED / SOURCE-TRACED BY FAMILY | Module/source-shape tests, process harnesses, oracle indexed path and manually joined product-spine path were classified. Causal tests move to receiving owners. | [`docs/implementation/TEST_CUTOVER.md`](../implementation/TEST_CUTOVER.md) |
| Tooling, swarm and generated governance | SOURCE-TRACED BY FAMILY | xtask commands, maps, ledgers, integration bootstrap, workflows, partial Rust/Qdrant analyser and P00 control plane were traced with their consumers. | [`xtask/README.md`](../../xtask/README.md) |
| Optional model/document/overlay/MCP/code-intelligence/evaluation | OPTIONAL | Each area is deleted, deferred or constrained to one selected profile/consumer. None blocks lexical Search. | [`docs/implementation/README.md`](../implementation/README.md) §11 |
| Donors | VERIFIED PER RECORDED SEAM | Concrete mechanisms and limitations were checked for redb, Qdrant, Tokio, Serde/TOML, ripgrep, Git, Tantivy/Lucene, Docling/Unstructured/PDFium, ast-grep/SCIP and selected RAG systems. This is not an exhaustive market survey. | [`DONOR_VERIFICATION_REGISTER_2026-10-09.md`](DONOR_VERIFICATION_REGISTER_2026-10-09.md) |
| Native Windows, live Qdrant, crash recovery, scale and performance | NOT EXECUTED | These become implementation/runtime evidence after the connected product path exists. | Implementation §14 |
| Private #598 branch/worktrees | OUTSIDE AUDIT | Only its public issue contract was used. No unpublished work was inspected or duplicated. | [#598](https://github.com/UnknownAlienHuman/eliot-search/issues/598) |

## 3. Documentation authority

The only project-wide documents allowed to direct implementation are:

1. [`docs/architecture/README.md`](../architecture/README.md) — concise purpose, boundaries and principles.
2. [`docs/implementation/README.md`](../implementation/README.md) — concrete owners, cutovers, order and donor seams.
3. [`docs/handoff/DEPENDENCY_GRAPH.md`](../handoff/DEPENDENCY_GRAPH.md) — current effect/data dependency graph.
4. This coverage map.
5. [`WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md`](WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md) — all workspace members.
6. Current package-local `README.md`, `FUNCTIONS.md` and `AGENTS.md` where they agree with items 1–5.
7. The current active issue body for the implementation slice.

Supporting evidence:

- [`ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md`](ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md) — distilled source findings and reasons.
- [`docs/implementation/TEST_CUTOVER.md`](../implementation/TEST_CUTOVER.md) — test/harness disposition.
- donor register — only for explicitly recorded donor behavior/limits.

Historical-only path classes:

```text
docs/architecture/ELIOT_SEARCH_8.4_*
docs/audit/* outside README/coverage/disposition/distilled-audit/donor register
docs/handoff/* except README.md and DEPENDENCY_GRAPH.md
docs/product/PACKAGE_STATUS.toml and the old analysis framework
swarm/**
qualification/**
config/build-profiles-v1.toml
config/data-layout-v1.toml
retired package maps, coverage graphs and accepted-evidence records
```

These files may preserve evidence but cannot issue a current `MUST`, block a cutover or restore old
compatibility/process machinery. [`docs/audit/README.md`](README.md),
[`docs/handoff/README.md`](../handoff/README.md) and [`docs/product/README.md`](../product/README.md) make the
path-level rule explicit. Git history is the archive; individual historical files need not be rewritten to
mirror the new architecture.

## 4. Documentation and queue reconciliation completed

- [x] Old master audit replaced by an explicit historical pointer.
- [x] 2026-10-17 audit distilled and corrected; S11 v1 is no longer preserved.
- [x] All 52 workspace members mapped.
- [x] Audit, handoff and product entrypoints define current versus historical authority.
- [x] High-risk current package instructions checked: root, config, source, control, preparation, identity,
      Qdrant, publication, scope/query/evidence, windows, protocol/client/CLI, contracts, optional packages,
      retention/tooling.
- [x] Active implementation queue reconciled: #214, #244, #248, #249, #259, #261, #263, #264, #266, #267,
      #270–#280, #291, #293, #300, #302, #303, #329–#333, #335, #360 and #525 describe the current owner/data
      model and do not retain old blockers.
- [x] Optional areas explicitly deleted/deferred/selected; they cannot block baseline.
- [x] Static findings are separated from unexecuted runtime claims.

Old issue links to immutable intermediate documentation commits remain valid evidence; architecture,
implementation and this map override them if wording differs. There is no need to churn every issue merely
to replace a stable commit URL.

## 5. Closure and next step

The **architecture/code/donor/documentation audit is complete for beginning the refactor**. It remains a broad
source-traced audit, not a line-by-line verification of all 1,593 Rust files. Compiler-guided removal will
expose individual residual callers during implementation, and each cutover must delete its replaced package,
tests, docs, tooling and dependency edges.

The only legitimate pre-code dependency is the accepted public handoff from #598 for its owned source/object/
key boundary. Everything else has a target owner and implementation issue.

First Rust slice:

```text
#598 accepted retained-source result
→ #330 one immutable object owner
→ #270 one current SourceRecord / RevisionRef compare-set
→ #331 one preparation handoff
```

Then follow the implementation order in [`docs/implementation/README.md`](../implementation/README.md). No
additional audit wave, launch gate, package-status update, compatibility plan or qualification receipt is
required before coding.
