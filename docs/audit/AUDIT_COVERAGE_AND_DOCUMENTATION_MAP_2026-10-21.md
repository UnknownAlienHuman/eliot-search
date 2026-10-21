# ELIOT Search — audit coverage and documentation disposition

**Audited public baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Audit branch at map creation:** `49db5723f3896b6ca78c444c50e0ae9622273912`  
**Status:** refactor preparation; no product/runtime qualification.

This is the single map of what the current audit did and did not establish. It prevents a broad static
review from being represented as a line-by-line audit or runtime verification, and prevents historical
Architecture 8.4 documents from regaining implementation authority.

## 1. Status vocabulary

| Status | Meaning |
|---|---|
| **SOURCE-TRACED** | Primary implementations, entrypoints, real callers and competing owners were inspected deeply enough to make the recorded ownership/deletion decision. It does **not** mean every line or fixture was read. |
| **PARTIAL** | Important implementations or callers were inspected, but a remaining owner, optional branch or active private handoff prevents a final disposition. |
| **INVENTORIED** | Files, packages or generated state were measured/classified, but their implementation was not reviewed line by line. |
| **NOT EXECUTED** | No native/runtime/live-Qdrant/performance/recovery claim exists. |
| **OUTSIDE AUDIT** | Deliberately not inspected, normally because another active manager owns unpublished work. |

A subsystem is not "fully audited" merely because its README, traits, constructors or tests were read. A
refactoring decision is valid only where the real producer, real consumer and replacement/deletion boundary
were traced.

## 2. Source audit coverage

| Area | Status | What was checked | Remaining uncertainty / required follow-up | Current implementation map |
|---|---|---|---|---|
| Architecture, ADRs and old requirements | SOURCE-TRACED | Part I, accepted ADRs, Architecture 8.4 master, execution/wave/qualification requirements and conflicts with the local product | Continue detecting stale normative language outside the paths classified historical below | [`docs/architecture/README.md`](../architecture/README.md) |
| Workspace, Cargo topology, features and entrypoints | SOURCE-TRACED | Workspace members, daemon/CLI targets, default features, harness binaries, disconnected entrypoints and shadowed applications | Compiler validation after Rust cutovers; no current build claim | [`docs/handoff/DEPENDENCY_GRAPH.md`](../handoff/DEPENDENCY_GRAPH.md) |
| Root ownership and daemon owner composition | SOURCE-TRACED | Root open/init/inspect, lock/owner machinery, daemon composition, create-on-read and migration/quarantine paths | Windows/native behavior not executed | [`search-runtime-owner/FUNCTIONS.md`](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md) |
| Configuration, startup and readiness | SOURCE-TRACED | Typed model, custom parser/tree/diff/action/receipt layers, startup rejection of valid non-default settings and readiness flags | Actual simplified settings implementation not built | [`search-config/FUNCTIONS.md`](../../crates/search-config/FUNCTIONS.md) |
| Source admission, stable read, registry, history and identity | SOURCE-TRACED | Native acquisition, admission order, dropped body, registry replay/original inputs, source identity/history packages, sync refusal and reconcile scaffolding | #598 result must be consumed without duplicating its writer | [`crates/search-source/README.md`](../../crates/search-source/README.md) |
| Immutable objects, storage profile and Windows secrets | PARTIAL | Public revision stores, duplicate publication kernels, Windows durability gap, legacy DPAPI/DEK/pairing surfaces and unused crypto package | Active private #598 source/object/key work is OUTSIDE AUDIT; final keep/delete decision waits for accepted handoff | [`search-revision-store/FUNCTIONS.md`](../../crates/search-source/search-revision-store/FUNCTIONS.md) |
| Materialization, unitization and preparation | SOURCE-TRACED | Markdown/JATS/plain preparation, repeated parse/materialization, duplicate artifact writer, ExactBytes restrictions, UnitSet serialization/copying and structure-first donor mechanisms | No runtime corpus/throughput measurements | [`search-materializer/FUNCTIONS.md`](../../crates/search-prep/search-materializer/FUNCTIONS.md) |
| Retrieval point identity, projection and manifests | SOURCE-TRACED | Legacy identity, S11 v1, point collision/readback mechanics, revision/offset rewrite amplification, projection planner and full-plan copying | New stable identity/profile must be implemented and measured on edits | [`search-point-identity/README.md`](../../crates/search-index-qdrant/search-point-identity/README.md) |
| Qdrant supervisor, bridge, schema, mutations and query | SOURCE-TRACED | Real SDK plane, blocking runtime, qualification gate, auth lease, process-local receipt ledger, payload/vector semantics, BM25/IDF/fusion donor APIs and disconnected daemon callers | No live Qdrant execution; topology/performance remain NOT EXECUTED | [`search-qdrant-bridge/FUNCTIONS.md`](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) |
| Publication, view lifetime, cleanup and GC | SOURCE-TRACED | Coordinator/daemon publisher duplication, pending intent recovery gap, pins/watermarks/reclaimer/retention systems, cleanup thresholds, backup/restore and object-GC roots | Crash/restart ordering remains design-only until code and fault execution | [`search-publication/FUNCTIONS.md`](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) |
| Query access, planning, execution, validation, projection and exact scan | SOURCE-TRACED | Recipe/capability/plan/grant stack, scope compilation, executor tickets/scheduler/count, fusion identity, readback proof defect, projector reorder and exact proof platform | Real connected query not executed | [`crates/search-query/README.md`](../../crates/search-query/README.md) |
| Handles, continuation, paging and expansion | SOURCE-TRACED | Two shared stores plus two daemon stores, whole-catalog fences, prefix rescans, arbitrary revision expansion and useful commit-after-output behavior | QueryWindow implementation and output fault behavior not executed | [`search-continuation/FUNCTIONS.md`](../../crates/search-query/search-continuation/FUNCTIONS.md) |
| Protocol, provider client, transports and CLI | SOURCE-TRACED | Pairing/MAC/grant/manual JSON stacks, typed client, daemon provider composition, public line protocol, legacy tab/hex path and child-stdio owner | Current framed protocol not implemented or executed | [`search-provider-protocol/FUNCTIONS.md`](../../crates/search-provider-protocol/FUNCTIONS.md) |
| Shared contracts, domain and global ports | SOURCE-TRACED | Contract families, recipes/plans/proofs, 23 global port traits/80 operations, domain operations and external production caller inventories | Not every type implementation was reviewed line by line; cutover requires compiler-guided removal | [`search-contracts/README.md`](../../crates/search-contracts/README.md) |
| Tests, harnesses and process proofs | INVENTORIED / SOURCE-TRACED BY FAMILY | Daemon test families, module-ownership tests, process binaries, oracle-based indexed query and manually joined product-spine harness | Individual causal tests must be reclassified during each Rust deletion slice | [`docs/implementation/TEST_CUTOVER.md`](../implementation/TEST_CUTOVER.md) |
| Tooling, generated governance and swarm records | SOURCE-TRACED BY FAMILY | xtask commands, coverage/maps, digest ledger, integration bootstrap, workflows, source-shape/Qdrant analyser and P00 control plane | Exact removal closure must be compiler/search driven when Rust/data/workflows are deleted | [`xtask/README.md`](../../xtask/README.md) |
| Optional model/document/overlay/MCP/code-intelligence/evaluation | PARTIAL | Main scaffolds, callers, refusal binaries, selected donor APIs and whether each blocks baseline | No exhaustive review of every optional implementation/donor; each remains optional until a real consumer exists | [`docs/implementation/README.md`](../implementation/README.md) §11 |
| Donors | PARTIAL / VERIFIED PER RECORDED SEAM | Concrete source/docs for redb, Qdrant, Tokio, Serde/TOML, ripgrep, Git plumbing, Tantivy/Lucene, Docling/Unstructured/PDFium, ast-grep/SCIP and selected RAG systems | This is not an exhaustive market survey; only recorded mechanisms are accepted | [`DONOR_VERIFICATION_REGISTER_2026-10-09.md`](DONOR_VERIFICATION_REGISTER_2026-10-09.md) |
| Native Windows, live Qdrant, crash recovery, scale and performance | NOT EXECUTED | Source and donor behavior only | Must be executed after the connected path exists; no source document may claim qualification | Implementation §14 |
| Private #598 branch/worktrees | OUTSIDE AUDIT | Public issue contract only | Do not inspect/reset/rebase/duplicate unpublished work; consume accepted handoff later | [#598](https://github.com/UnknownAlienHuman/eliot-search/issues/598) |

## 3. Documentation authority

### Current normative documents

These are the only project-wide documents allowed to direct implementation:

1. [`docs/architecture/README.md`](../architecture/README.md) — concise purpose, boundaries and principles.
2. [`docs/implementation/README.md`](../implementation/README.md) — concrete owners, cutovers, order and donor seams.
3. [`docs/handoff/DEPENDENCY_GRAPH.md`](../handoff/DEPENDENCY_GRAPH.md) — current effect/data dependency graph.
4. This coverage/disposition map — audit completeness and document authority.
5. Current package-local `README.md`, `FUNCTIONS.md` and `AGENTS.md` only where they agree with items 1–4.
6. Current GitHub issue body for the active implementation slice; an old issue description cannot override items 1–4.

The donor register is authoritative only for the donor behavior and limits it explicitly records. It does
not define product ownership.

### Supporting evidence, not implementation authority

- [`ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md`](ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md) — distilled source findings and reasons for cutovers.
- [`docs/implementation/TEST_CUTOVER.md`](../implementation/TEST_CUTOVER.md) — test/harness disposition.
- Older audits, benchmarks and inventory files may support a claim only when their base and limitation are stated.

### Historical-only path classes

The following may be read for requirements, defects or donor history but cannot issue a current `MUST`,
block a cutover or restore compatibility/process machinery:

```text
docs/architecture/ELIOT_SEARCH_8.4_*
docs/audit/* before the current coverage/audit pair, unless explicitly referenced as evidence
docs/handoff/P00_* and wave/gate/lease/handoff control-plane documents
docs/product/PACKAGE_STATUS.toml
swarm/**
qualification/**
config/build-profiles-v1.toml
config/data-layout-v1.toml
retired package maps, coverage graphs and accepted-evidence records
```

Historical documents should be shortened to an explicit pointer or deleted with their final consumer. Git
history is the archive; do not create a second archive directory.

## 4. Known contradictions at map creation

These are defects in the documentation branch, not future Rust work:

1. `ELIOT_SEARCH_MASTER_AUDIT_2026-10-09.md` still calls itself the main entrypoint and prescribes old package-status, wave and migration work.
2. The 2026-10-17 audit still says to preserve the delivered S11 API, while the current point-identity map correctly replaces revision/coordinate-bound S11 v1.
3. PR #610 links and scope statistics lag its actual head.
4. Many old audits/qualification/handoff files remain discoverable without a prominent historical marker.

The current documentation pass must fix items 1–3 immediately and continue item 4 by path class rather than
editing hundreds of historical files individually.

## 5. Remaining audit work

The audit is **not complete** until all of the following are true:

- [ ] Every workspace member has one disposition: **keep**, **merge/inline**, **delete**, **optional selected profile**, or **outside active handoff**.
- [ ] Every project-wide document is classified by the authority rules above; no old document calls itself current or directs agents into retired mechanisms.
- [ ] Current package-local instructions contain no requirement which contradicts architecture/implementation.
- [ ] Active implementation issues use the current owner/data model and do not retain obsolete blockers.
- [ ] Remaining optional areas are either explicitly deferred/deleted or receive a selected concrete implementation.
- [ ] Runtime/native/Qdrant/recovery/performance claims are separated from static findings and executed only after the connected product path exists.
- [ ] The final audit summary names remaining unreviewed source families instead of claiming complete line-by-line coverage.

## 6. Immediate audit sequence

1. Replace the obsolete master audit with a historical pointer.
2. Distill the 2026-10-17 audit and correct decisions which changed during the deeper source review.
3. Build a workspace-member disposition table from current Cargo metadata/source callers.
4. Sweep current package instructions for old compatibility/security/receipt/qualification requirements.
5. Sweep old project-wide documents for false "current/main/required" status and classify/delete them by path.
6. Reconcile the active issue queue against the final implementation order.
7. Only then declare documentation preparation complete and begin the first Rust cutover.

No percentage-complete claim is made. The map records evidence and remaining closure conditions instead of a
plausible-looking number.
