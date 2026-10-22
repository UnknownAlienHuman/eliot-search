# Analysis registry

This file records what the refactor audit actually inspected, what changed and what remains. It is a memory aid, not an approval gate, package-status system or substitute for the owning implementation documents.

**Public source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Historical qualification archive:** `2f9c9720f3e14799a0f8dfefa00077d2fa6a9887/qualification`  
**Update rule:** every audit/cleanup pass changes this registry with exact paths, findings, remaining work and confidence. Never infer a completion percentage from row counts.

## Status vocabulary

- **CHECKED** — the named static surface and principal callers were inspected for the stated question. It does not mean every line or runtime behavior was proved.
- **PARTIAL** — useful evidence exists, but named files/callers or reconciliation work remain.
- **PENDING** — not yet inspected enough to direct deletion or implementation.
- **RUNTIME-ONLY** — static inspection cannot establish the claim; implementation and execution are required.
- **PROTECTED** — private/unpublished work must not be inspected or overwritten; only its accepted public handoff can be consumed.

## Coverage ledger

| ID | Area | Exact inspected surface | Status | Finding / change | Remaining work | Confidence |
|---|---|---|---|---|---|---|
| A001 | Documentation authority | `AGENTS.md`; `docs/architecture/README.md`; `docs/implementation/README.md`; `docs/README.md`; `docs/handoff/{README,DEPENDENCY_GRAPH}.md`; audit entrypoints | CHECKED | One authority chain selected. Old architecture master, packets, status graph, handoffs and generated maps removed in `f3c6523f`, `48be68c9`, `a4af41f8`. | Recheck links whenever owner files are deleted or renamed. | High for static authority. |
| A002 | Historical governance data | `swarm/**`; generated package maps; packet/handoff trees; `artifacts/context-*`; `artifacts/ticket-*` | CHECKED | Current-tree copies removed; Git history is the archive. | Rust/tool consumers are tracked separately in A015. | High. |
| A003 | Workflows and wrapper tools | `.github/workflows/**`; `tools/*.ps1`; `tools/README.md` | CHECKED | Wave/map/evidence/bootstrap/source-shape workflows and wrappers removed in `a4af41f8`; one raw manual Cargo workflow remains. | Verify no Rust test or README still treats removed workflow output as authority. | High for files, medium for all code references. |
| A004 | Workspace/package inventory | root `Cargo.toml`; 52 package manifests; `WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md` | CHECKED | Every member has KEEP/MERGE/DELETE/OPTIONAL/CONDITIONAL disposition; this is not a target package count. | Physical Cargo/Rust cutovers and dependency removal remain implementation work. | High for inventory, medium for hidden feature/build consumers. |
| A005 | Entrypoints and runnable product | daemon/CLI entry composition; root `QUICKSTART.md`; daemon command interceptors and public runtime paths | CHECKED | Only bounded one-shot scan is a simple runnable path. Persistent source→Qdrant→evidence path is disconnected and command routing contains competing applications. | Recheck after the direct dispatcher is implemented; execute actual binary. | High static, runtime pending. |
| A006 | Root/config/control | runtime owner, config, generic control journal/snapshot, registry restore; current owner maps | CHECKED | Selected one root lock, typed current rows and finite pending external work; operation archive/snapshot rebuild is replaced. | Compile after typed-row cutover; verify Windows lock/durability. | High static. |
| A007 | Source acquisition and registry | admission/receipt/provenance/records/history/registry/reconcile; safe-reader; daemon native source path; issues #266/#267/#270/#271 | CHECKED | Exact acquired bytes must feed one object writer and one current head; full registry replay/original-input receipts are removal targets. | Actual writer integration depends on accepted #598 handoff; compile and crash cases. | High static. |
| A008 | Private storage/key overlap | public #598 issue contract and public-main storage/crypto boundaries only | PROTECTED | Public requirements were compared with current architecture; obsolete six-domain/purge/compatibility requirements are not inherited. | Consume accepted public handoff; do not inspect/reset/duplicate unpublished branch/worktrees. | High about boundary, unknown implementation. |
| A009 | Preparation/unit identity | materializer/unitizer implementations and callers; empty/gap cases; projection/point identity; issues #331/#259/#329 | CHECKED | One materialization, recoverable input/outcome comparison, stale-result compare/install, conservative one-old/one-new reuse and versioned evidence selected. | Implement and measure rewrite amplification/duplicate edits. | High static. |
| A010 | Qdrant bridge and donor behavior | real bridge constructors/callers; Qdrant 1.19 mutation/readback, native BM25 tokenizer/inference and scoped-IDF source; issues #263/#261 | CHECKED | Real adapter is disconnected; qualification credentials/process receipt ledger removed from target. Native IDF may scan broad population; punctuation-sensitive identity needs exact plane. | Remove remaining qualification-bound bridge/supervisor code with its callers; execute late-write/relevance/latency/scale runs. | High for inspected source; runtime-only performance. |
| A011 | Publication/view/cleanup | publication implementations, control intents/snapshots, epoch pins, reclaimer, retention, restore; issues #261/#300/#302/#303 | CHECKED | One pending publication/current view/unfinished cleanup owner selected. Old readers and possible late writers both constrain deletion; local object GC is stopped-root maintenance. | Implement and execute restart, delayed RPC, reader growth, backlog and restore/import cases. | High static. |
| A012 | Query/evidence/exact | planner/executor/access/validator/projector/handles/continuations; literal matcher; daemon process-test oracle; issues #249/#272/#279/#291/#293 | CHECKED | Direct handlers, one request context, manifest→retained evidence and explicit exact input domain selected; proof-shaped stack and duplicate stores are removal targets. | Implement real Qdrant/retained path and exact streaming cancellation. | High static. |
| A013 | Protocol/client/CLI | provider protocol/client, daemon provider/public runtime composition, CLI legacy/typed clients; issues #275/#280/#332 | CHECKED | One bounded framed session selected. One-shot lifetime, blocked-I/O cancellation, partial output and window accounting contradictions were corrected. | Implement and execute same-session Continue/Expand, cancellation and child cleanup. | High static. |
| A014 | Product depth | original orientation/comparison/document requirements; code enricher, subject resolver, comparator, PDF/model/overlay scaffolds; #281/#227/#336 | PARTIAL | Observable navigation, ambiguity, comparison and document behavior preserved; empty frameworks are not capabilities. | Inspect/implement only when a concrete producer/consumer is selected; evaluate quality on real corpora. | Medium. |
| A015 | Tests and repository governance code | `xtask/**`; module-ownership/source-shape tests; daemon harness inventory; references to `qualification/**`; #214/#525 | PARTIAL | Generated authority and wrapper surfaces are mapped for deletion. Searches found qualification consumers in `xtask` architecture/evidence/Qdrant validators and old package tests/docs, not a valid runtime authority. | Delete Rust xtask, digest ledger, qualification-bound source-shape fixtures and Cargo consumers; retain causal cases; run minimal Clippy. | High inventory, medium consumer closure. |
| A016 | Donors | redb 2.6.3 snapshots/transactions; Qdrant 1.19; Aho-Corasick matcher; selected Git/ripgrep/Tantivy/Lucene mechanisms in donor register | PARTIAL | Mechanisms and hidden costs separated from claimed product behavior. Donor API catalogues are not implementation checklists. | Revisit a donor only for a concrete owner decision; runtime claims remain unproved. | Medium-high for named mechanisms. |
| A017 | Qualification tree | Complete recursive inventory at tree `bebdb9fb6d8ea5adff96b2f6c19e008ef1e13b7e`; root README/CURRENT_FAILURES; Qdrant archive README; code-enrichment report; os-secrets tree; repository searches for `qualification/` consumers | CHECKED | The tree mixed W1-W10 milestones, agent drafts, PASS/FAIL summaries, gate maps, unavailable capabilities and historical raw reports. Root policy already declared them non-authoritative. Product references were old docs/tests or retired qualification code. Full archive retained at `2f9c9720`; current tree keeps only README and blocker summary. | Remove qualification-bound Rust/xtask consumers under A010/A015; future raw evidence belongs to its concrete issue/PR. | High for current-tree disposition, not a revalidation of every historical run. |
| A018 | Package-local auxiliary docs | `bins/eliot-search/W8_CLIENT.md`; provider `W8_HARDENING.md`; query `W7_HARDENING.md`; access snapshot/security notes; Qdrant `P18_SCALE.md`; control journal/intent/quarantine/checkpoint notes; query-window helper notes | PARTIAL | Checked files either required removed mechanisms or repeated current owner `FUNCTIONS.md`; removed in `2f9c9720`. | Enumerate remaining package-local `W*`, `P*`, `*HARDENING*`, receipt/journal/qualification docs and repair links. | High for checked files, unknown for remainder. |
| A019 | Open issue queue | Current implementation chain and obvious duplicate/superseded tasks through audit PR; #352 coordination | CHECKED | Active issues were reconciled to current owners; old harness/graph/lint/status tasks closed. | Refresh after each code cutover or when issue bodies change materially. | Medium-high. |
| A020 | Runtime evidence | Windows no-clobber/directory durability, blocked I/O cancellation, live Qdrant late writes, scoped-IDF cost, relevance, old-reader growth, protected restore, end-to-end restart | RUNTIME-ONLY | Static docs identify the required experiments and do not issue readiness. | Implement and execute on the named platform/backend; record raw results in owning issue, not a new qualification OS. | Not statically knowable. |

## Pass log

### R001 — authority/governance removal

- Removed the Architecture 8.4 master, wave/handoff/status/swarm/generated authority and redundant workflow/tool surfaces through `a4af41f8`.
- Result: implementation cannot be redirected by a parallel current-tree specification; historical evidence remains in Git.

### R002 — package-local wave remnants

Inspected and removed:

- `bins/eliot-search/W8_CLIENT.md`;
- `crates/search-provider-protocol/W8_HARDENING.md`;
- query continuation/handle/access/candidate-validator W7 notes;
- query-window helper notes already owned by `search-continuation/FUNCTIONS.md`;
- `search-qdrant-bridge/P18_SCALE.md`;
- control journal/cutover/intent/quarantine/checkpoint/successor/visible-epoch notes.

Finding: one file still prescribed removed pairing/grant/recipe behavior; the rest repeated current owner rules. Keeping them multiplied agent context and created two apparent owners for one algorithm. Commit: `2f9c9720`.

### R003 — qualification archive disposition

Inspected:

- complete recursive qualification tree `bebdb9fb6d8ea5adff96b2f6c19e008ef1e13b7e`;
- `qualification/{README.md,CURRENT_FAILURES.txt}`;
- `qualification/qdrant/README.md` and its archived artifact/schema/probe layout;
- `qualification/code-enrichment/{REPORT.md,ARTIFACT_PIN.toml}`;
- representative failure-only trees such as `control-redb`, `direct-runtime`, `os-secrets`, handles/eval/adapter;
- code searches for `qualification/` under `xtask` and production packages.

Finding: useful reports were exact historical evidence tied to old commits/tasks; structural files were wave/package/acceptance authority or unavailable placeholders. Actual consumers are the retired xtask/Qdrant qualification machinery and old docs/tests. No current product behavior should load the tree as authority.

Action: replace the current qualification tree with a concise README and current blocker summary. Preserve the complete archive in Git at `2f9c9720`. Record new execution evidence in the owning issue/PR, not a central runtime credential.

Next pass: remaining package-local auxiliary filenames and stale links, then xtask/Cargo/Rust consumer deletion.
