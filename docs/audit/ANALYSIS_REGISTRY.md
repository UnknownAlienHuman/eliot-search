# Analysis registry

This file records what the refactor audit actually inspected, what changed and what remains. It is a memory aid, not an approval gate, package-status system or substitute for the owning implementation documents.

**Public source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Historical qualification archive:** `2f9c9720f3e14799a0f8dfefa00077d2fa6a9887/qualification`  
**Update rule:** every audit or cleanup pass changes this registry with exact paths, findings, remaining work and confidence. Never infer a completion percentage from row counts.

## Status vocabulary

- **CHECKED** — the named static surface and principal callers were inspected for the stated question. It does not mean every line or runtime behavior was proved.
- **PARTIAL** — useful evidence exists, but named files, callers or reconciliation work remain.
- **PENDING** — not yet inspected enough to direct deletion or implementation.
- **RUNTIME-ONLY** — static inspection cannot establish the claim; implementation and execution are required.
- **PROTECTED** — private or unpublished work must not be inspected or overwritten; only its accepted public handoff can be consumed.

## Coverage ledger

| ID | Area | Exact inspected surface | Status | Finding / change | Remaining work | Confidence |
|---|---|---|---|---|---|---|
| A001 | Documentation authority | `AGENTS.md`; architecture; implementation; docs/handoff and audit entrypoints | CHECKED | One authority chain selected. Old architecture master, packets, status graph, handoffs and generated maps removed. | Recheck links whenever owner files move. | High static. |
| A002 | Historical governance data | `swarm/**`; generated maps; packet/handoff trees; context/ticket artifacts | CHECKED | Current-tree copies removed; Git history is the archive. | Rust/tool consumers are tracked under A015/A022. | High. |
| A003 | Workflows and wrappers | `.github/workflows/**`; `tools/*.ps1`; tools README | CHECKED | Wave, map, evidence, bootstrap and source-shape workflows/wrappers removed. One manual raw Cargo diagnostic remains. | Recheck only when a real workflow is added or changed. | High. |
| A004 | Workspace inventory | root Cargo manifest; 51 current package manifests; workspace disposition | CHECKED | Every current member has a receiving, removal, optional or conditional disposition; this is not a target package count. `xtask` is no longer a member. | Physical product-package cutovers. | High inventory. |
| A005 | Entrypoints/runnable product | daemon/CLI entry composition; command interceptors; quickstarts | CHECKED | Only bounded one-shot scan is a simple runnable path; persistent product path remains disconnected. | Recheck and execute after direct dispatcher. | High static. |
| A006 | Root/config/control | runtime owner; config; generic control journal/snapshot; registry restore | CHECKED | One root lock, typed current rows and finite unfinished external work selected. | Compile and test Windows lock/durability. | High static. |
| A007 | Source acquisition/registry | admission, receipts, provenance, records/history/registry/reconcile, safe reader, daemon source path | CHECKED | Exact acquired bytes must feed one object writer and current head; registry replay/original-input archive are removal targets. | Integrate accepted #598 handoff and crash cases. | High static. |
| A008 | Private storage/key overlap | public #598 issue contract and public-main boundaries only | PROTECTED | Obsolete six-domain, purge and compatibility programme is not inherited. | Consume only accepted public handoff; do not inspect, reset or duplicate private work. | High boundary, implementation unknown. |
| A009 | Preparation/identity | materializer, unitizer, empty/gap cases, projection and point identity; current S11 implementation/profile docs | CHECKED | One materialization, recoverable input/outcome comparison, stale-result guard, conservative occurrence reuse and versioned evidence selected. Point identity owner now states the replacement directly; revision-bound S11 is not compatibility. | Implement and measure rewrite amplification and duplicate edits. | High static. |
| A010 | Qdrant/donor behavior | real bridge callers; Qdrant 1.19 mutation/readback/BM25/tokenizer/scoped-IDF source | CHECKED | Real adapter is disconnected. IDF may scan a broad population; exact identifiers need a separate plane. Qualification credentials are not part of the target, but code remnants are tracked in A022. | Run live late-write, relevance and scale cases after the direct bridge cutover. | High inspected source. |
| A011 | Publication/view/cleanup | competing publication machines; control intents; epoch pins; reclaimer; retention; restore | CHECKED | One pending/current/unfinished-cleanup owner selected. Reader drain and possible late writers both constrain Qdrant deletion. Local object GC is stopped-root maintenance. | Implement delayed-RPC, reader-growth, backlog, GC and restore/import cases. | High static. |
| A012 | Query/evidence/exact | planner/executor/access/validator/projector/handles/continuations; literal matcher; oracle path | CHECKED | Direct handlers, one request context, retained evidence and explicit exact input semantics selected. | Implement real Qdrant/evidence path and streaming cancellation. | High static. |
| A013 | Protocol/client/CLI | provider protocol/client; daemon provider/public runtime; CLI legacy/typed stacks | CHECKED | One framed live session selected; dead locators, blocked-I/O cancellation, partial output and cursor-accounting contradictions corrected. | Implement and execute same-session Continue/Expand/cancel/child cleanup. | High static. |
| A014 | Product depth | old orientation/comparison/document requirements; enrichers/resolvers/comparator/workers/overlay | PARTIAL | Observable user behavior preserved; scaffolds are not capabilities. | Select concrete producers/consumers and evaluate quality. | Medium. |
| A015 | Tests/governance code | `xtask/**`; workspace/lock/workflows; product `*ownership.rs`; source-text tests | PARTIAL | `xtask`, the 3.57 MB ledger and exclusive lock dependencies are gone. Removed 11 Qdrant, 5 evaluation, 33 daemon and 17 preparation/store source-shape tests while retaining behavior suites. Runs `39868374272`, `39869930525` and `39870086718` passed locked metadata and strict Clippy for the changed owners. | Find remaining tests that inspect source text, file names or retired documents without exercising behavior. | High for removed surfaces, medium for remaining search. |
| A016 | Donors | redb 2.6.3; Qdrant 1.19; Aho-Corasick; selected Git/ripgrep/Tantivy/Lucene mechanics | PARTIAL | Mechanisms and hidden costs separated from product claims; API catalogues are not implementation checklists. | Revisit only for concrete owner decisions. | Medium-high named mechanisms. |
| A017 | Qualification tree | Complete tree inventory; root policy/blockers; Qdrant/code-enrichment/os-secret samples; consumer searches | CHECKED | Old W1-W10, gate and PASS/FAIL database archived at `2f9c9720`; current tree retains only policy and blocker summary. | Runtime approval semantics that survived in Rust are tracked in A022. | High disposition. |
| A018 | Package-local auxiliary docs | Complete current-tree package/binary root sweep; supplement-name set; nonstandard journal/profile/DIRECT/helper files | CHECKED | Removed W7/W8/W10/P18 supplements, copied window/control/profile files, four daemon DIRECT/source-mapping documents and duplicate TOML replacement specification. Unique current `DIAGNOSTICS.md` remains. | Recheck only when a package adds or renames an auxiliary owner document. | High static. |
| A019 | Open issues | Current implementation chain and duplicate/superseded tasks; #352, #214, #525 | CHECKED | Issues reconciled to current owners; old harness/graph/lint/status tasks closed. #525 is complete; #214 remains open for final code consumers. | Refresh after material code or issue changes. | Medium-high. |
| A020 | Runtime evidence | Windows durability/cancellation; live Qdrant late writes/IDF; relevance; reader growth; protected restore; restart | RUNTIME-ONLY | Static docs identify required experiments and issue no readiness. | Implement and execute; record raw results with owning issue or PR. | Not statically knowable. |
| A021 | Compiler baseline | Ubuntu Rust 1.98 runs `39867915623`, `39868025680`, `39868205817`, `39868374272`, `39869930525`, `39870086718` | PARTIAL | Tooling and affected-owner cuts resolve cleanly. A broad `cargo check -p eliot-searchd` exposed pre-existing legacy/target-bound errors: missing unsupported modules, unconditional Windows-secret imports, missing `std::fs::File` and ambiguous associated types. These are not caused by the cleanup. | Assign each daemon error to its owner cutover, remove obsolete paths instead of patching dead architecture, then rerun daemon and Windows checks. | High observed evidence; product build remains failing. |
| A022 | Runtime qualification remnants | Qdrant `qualified/**`, live suite, `RealDataPlane::connect`; supervisor artifact identity; lexical code-profile fixtures/receipts | PARTIAL | The external qualification database is gone, but Qdrant still accepts `QualifiedGate` as a constructor token and lexical code paths still mint qualification receipts from embedded fixtures. Current probes and causal fixtures may be useful; runtime approval semantics are not. | Trace production callers and replace credentials with selected configuration plus current observation. Keep causal fixtures as tests, not activation authority. | High static finding; implementation decision pending. |

## Pass log

### R001 — authority/governance removal

Removed the Architecture 8.4 master, wave/handoff/status/swarm/generated authority and redundant workflow/tool surfaces through `a4af41f8`. Historical evidence remains in Git.

### R002 — first package-local wave cleanup

Inspected and removed CLI/provider W8, query W7, query-window helpers, Qdrant bridge P18 and control journal/cutover/intent/quarantine/checkpoint copies. They either prescribed removed mechanisms or repeated current owner rules. Commit: `2f9c9720`.

### R003 — qualification archive disposition

Inspected the complete qualification tree, representative raw reports and code consumers. Replaced the current tree with a policy and current blocker summary; exact archive remains at `2f9c9720`. Commit: `49f5c46b`.

### R004 — standard supplement closure and GC contradiction

Inspected the supplement naming policy in the old xtask and every standard supplement remaining on the branch. The daemon W8/W10 files still ordered grants, accepted receipts, capability snapshots, worker qualification, quarantine and migration/rollback state. The others copied current publication/view/cleanup/evaluation summaries.

Owner verification found a replacement error in `search-index-reclaimer/FUNCTIONS.md`: its local artifact section still prescribed online shared/exclusive GC while current implementation selected explicit stopped-root maintenance. The owner map was rewritten: publication owns Qdrant cleanup; local object collection runs only with daemon, producers and windows stopped under one exclusive root lock. Standard supplements were removed in `140852e4`.

### R005 — nonstandard package-local closure

Inspected current package/binary roots rather than default-branch search results. Checked arbitrary uppercase/current-looking documents including journal, checkpoint, quarantine, recovery, profile, integration, scale, security and snapshot names.

Findings:

- `crates/search-control-redb/SECURITY_RESTRICTION_JOURNAL.md` was another historical explanation of a removed policy journal; its current rule already belongs to `search-access` and control `FUNCTIONS.md`.
- `crates/search-index-qdrant/search-point-identity/S11_PROFILE.md` contained useful replacement reasoning, but `FUNCTIONS.md` simultaneously froze the old S11 public surface. The replacement identity, collision rule, ambiguous-occurrence policy and cutover now live directly in `FUNCTIONS.md`; README is only an entrypoint.
- `crates/search-control-redb/DIAGNOSTICS.md` is retained because it defines a distinct current read-only operation rather than a historical gate or copied state machine.

Action: remove the redundant journal/profile files and mark their current owners explicitly.

### R006 — xtask and digest-governance removal

Inspected the root manifest, lockfile, aliases, workflows and complete `xtask` command/module surface. No current product operation remained: every command generated or validated removed accepted-evidence, architecture coverage, package maps, integration bootstrap, Qdrant source boundaries or the canonical digest ledger.

Actions:

- `f9275731`: delete `xtask/**`, including the 3.57 MB ledger; remove the workspace member and `cargo_metadata` workspace dependency;
- `9d2fe65a`: regenerate `Cargo.lock`, removing xtask-exclusive packages;
- run `39868374272`: verify removed directory/package/dependencies, locked metadata, `search-contracts` check and strict Clippy under Rust 1.98;
- remove the temporary verifier after success.

A broad daemon check was recorded rather than concealed. Its old Linux/Windows composition already fails independently of this cut; A021 owns that evidence.

### R007 — source-shape test removal

Inspected and removed tests that read Rust files as text, enforced module names or byte ceilings, or froze legacy ownership placement rather than executing behavior:

- 11 Qdrant bridge module-ownership tests (`07c90c14`);
- 5 evaluation module-ownership tests (`30d1921f`);
- 33 daemon module-ownership tests plus four residual DIRECT/source-mapping documents (`789d4fb4`);
- 17 materializer, unitizer and revision-store ownership tests (`a0de1610`).

Retained CRUD, readback, recovery, IDF, metrics and materialization behavior suites. Run `39869930525` passed locked check and strict Clippy for `search-materializer`, `search-unitizer`, `search-revision-store`, `search-qdrant-bridge` and `search-eval`.

### R008 — active stale-reference and qualification-remnant sweep

Run `39870086718` searched active Markdown, Rust and Cargo surfaces after the deletions. It found stale references to removed xtask, package status and Qdrant qualification files. The affected current documents/comments were corrected; duplicate `TOML_PROFILE_2.md` was removed instead of maintaining a second config specification; ADR 0005 and the optional Research adapter were distilled.

The same pass exposed a deeper remaining code issue: qualification files are gone, but Qdrant still requires `QualifiedGate` and lexical code-profile paths still issue fixture-backed qualification receipts. A022 tracks that code cut separately. Useful current probes or fixtures are not deleted merely because their former approval semantics were wrong.
