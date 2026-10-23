# Analysis registry

This file records what the refactor audit actually inspected, what changed and what remains. It is a memory aid, not an approval gate, package-status system or substitute for the owning implementation documents.

**Original public source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Latest inspected source revision:** `7d92fb8b5532b5928bdb90ddd42d5685e5f01d48` (2026-10-23, R009).  
**Historical qualification archive:** `2f9c9720f3e14799a0f8dfefa00077d2fa6a9887/qualification`  
**Update rule:** commit findings and this registry together. On resume, read the actual PR head and intervening changes before trusting an older row. Bind every execution result to its SHA and command/target; never infer a completion percentage from row counts.

## Status vocabulary

- **CHECKED** — the named static surface and principal callers were inspected for the stated question. It does not mean every line or runtime behavior was proved.
- **PARTIAL** — useful evidence exists, but named files, callers or reconciliation work remain.
- **PENDING** — not yet inspected enough to direct deletion or implementation.
- **RUNTIME-ONLY** — static inspection cannot establish the claim; implementation and execution are required.
- **PROTECTED** — private or unpublished work must not be inspected or overwritten; only its accepted public handoff can be consumed.

## Coverage ledger

| ID | Area | Exact inspected surface | Status | Finding / change | Remaining work | Confidence |
|---|---|---|---|---|---|---|
| A001 | Documentation authority and consistency | `AGENTS.md`; architecture; implementation; docs/handoff and audit entrypoints; bridge README/AGENTS/FUNCTIONS | PARTIAL | Root authority selected and historical controllers removed. R009 found that current bridge README/AGENTS still contradicted FUNCTIONS; reduced them to entrypoints and corrected implemented-state claims. | Reconcile remaining current owner READMEs/AGENTS, not just historical filenames. Do not equate file removal with semantic consistency. | High for named findings, incomplete cross-owner sweep. |
| A002 | Historical governance data | `swarm/**`; generated maps; packet/handoff trees; context/ticket artifacts | CHECKED | Current-tree copies removed; Git history is the archive. | Rust/tool consumers are tracked under A015/A022. | High. |
| A003 | Workflows and wrappers | `.github/workflows/**`; `tools/*.ps1`; tools README; A022 workflow and run | CHECKED | Wave/map/evidence/bootstrap/source-shape workflows removed. R009 removes the leftover push-triggered A022 verifier after recording its result; one manual raw Cargo diagnostic remains. | Keep future checks explicit; do not leave a temporary automatic workflow behind after interruption. | High for inspected tree. |
| A004 | Workspace inventory | root Cargo manifest; 51 current package manifests; workspace disposition | CHECKED | Every current member has a receiving, removal, optional or conditional disposition; this is not a target package count. `xtask` is no longer a member. | Physical product-package cutovers. | High inventory. |
| A005 | Entrypoints/runnable product | daemon/CLI entry composition; command interceptors; quickstarts | CHECKED | Only bounded one-shot scan is a simple runnable path; persistent product path remains disconnected. | Recheck and execute after direct dispatcher. | High static. |
| A006 | Root/config/control | runtime owner; config; generic control journal/snapshot; registry restore | CHECKED | One root lock, typed current rows and finite unfinished external work selected. | Compile and test Windows lock/durability. | High static. |
| A007 | Source acquisition/registry | admission, receipts, provenance, records/history/registry/reconcile, safe reader, daemon source path | CHECKED | Exact acquired bytes must feed one object writer and current head; registry replay/original-input archive are removal targets. | Integrate accepted #598 handoff and crash cases. | High static. |
| A008 | Private storage/key overlap | public #598 issue contract and public-main boundaries only | PROTECTED | Obsolete six-domain, purge and compatibility programme is not inherited. | Consume only accepted public handoff; do not inspect, reset or duplicate private work. | High boundary, implementation unknown. |
| A009 | Preparation/identity | materializer, unitizer, empty/gap cases, projection and point identity; current S11 implementation/profile docs | CHECKED | One materialization, recoverable input/outcome comparison, stale-result guard, conservative occurrence reuse and versioned evidence selected. Revision-bound S11 is not compatibility. | Implement and measure rewrite amplification and duplicate edits. Reconsider refused preparation on processing-policy changes without invalidating unchanged successful retrieval inputs. | High static. |
| A010 | Qdrant/donor behavior | real bridge callers; Qdrant 1.19 mutation/readback/BM25/tokenizer/scoped-IDF source; current real.rs/connect/gate | CHECKED for named static questions | Real adapter remains disconnected from normal product flow. QualifiedGate constructor input is removed at `7d92fb8b`; fixed build/auth/deadline/ledger machinery remains. IDF cost and exact identifier limits remain explicit. | Complete direct bridge/publication/query cutover, then live late-write, relevance and scale checks. | High inspected source; not product execution. |
| A011 | Publication/view/cleanup | competing publication machines; control intents; epoch pins; reclaimer; retention; restore | CHECKED | One pending/current/unfinished-cleanup owner selected. Reader drain and possible late writers both constrain Qdrant deletion. Local object GC is stopped-root maintenance. | Implement delayed-RPC, reader-growth, backlog, GC and restore/import cases. | High static. |
| A012 | Query/evidence/exact | planner/executor/access/validator/projector/handles/continuations; literal matcher; oracle path | CHECKED | Direct handlers, one request context, retained evidence and explicit exact input semantics selected. | Implement real Qdrant/evidence path and streaming cancellation. | High static. |
| A013 | Protocol/client/CLI | provider protocol/client; daemon provider/public runtime; CLI legacy/typed stacks | CHECKED | One framed live session selected; dead locators, blocked-I/O cancellation, partial output and cursor-accounting contradictions corrected. | Implement and execute same-session Continue/Expand/cancel/child cleanup. | High static. |
| A014 | Product depth | old orientation/comparison/document requirements; enrichers/resolvers/comparator/workers/overlay | PARTIAL | Observable user behavior preserved; scaffolds are not capabilities. | Select concrete producers/consumers and evaluate quality. | Medium. |
| A015 | Tests/governance code | `xtask/**`; workspace/lock/workflows; product `*ownership.rs`; source-text tests | PARTIAL | `xtask`, the 3.57 MB ledger and exclusive lock dependencies are gone. Removed 11 Qdrant, 5 evaluation, 33 daemon and 17 preparation/store source-shape tests while retaining behavior suites. Runs `39868374272`, `39869930525` and `39870086718` passed their recorded checks. | Find remaining source/file/document assertions without behavioral coverage. A previous run proves only its recorded revision and target set. | High for removed surfaces, medium for remaining search. |
| A016 | Donors | redb 2.6.3; Qdrant 1.19; Aho-Corasick; selected Git/ripgrep/Tantivy/Lucene mechanics | PARTIAL | Mechanisms and hidden costs separated from product claims; API catalogues are not implementation checklists. | Revisit only for concrete owner decisions. | Medium-high named mechanisms. |
| A017 | Qualification tree | Complete tree inventory; root policy/blockers; Qdrant/code-enrichment/os-secret samples; consumer searches | CHECKED | Old W1-W10, gate and PASS/FAIL database archived at `2f9c9720`; current tree retains only policy and blocker summary. | Rust approval semantics are tracked in A022; archived data removal did not remove them. | High disposition. |
| A018 | Package-local auxiliary filenames | Current package/binary root sweep; supplement-name set; nonstandard journal/profile/DIRECT/helper files | CHECKED for filename/disposition scope | Removed W7/W8/W10/P18 supplements, copied window/control/profile files, four daemon DIRECT/source-mapping documents and duplicate TOML replacement specification. Unique current DIAGNOSTICS remains. | Semantic consistency of surviving README/AGENTS/FUNCTIONS is A001, not implied by this row. | High stated scope. |
| A019 | Open issues/PR handoff | Current implementation chain and duplicate/superseded tasks; #352, #214, #525; PR #610 | PARTIAL | Old harness/graph/lint/status tasks closed. #525 is complete; #214 remains open. R009 found PR description and registry behind already-published code and corrected the checkpoint. | Refresh material owner changes in current issues; no claim that every open issue was reread in R009. | High named checkpoint, incomplete queue reread. |
| A020 | Runtime evidence | Windows durability/cancellation; live Qdrant late writes/IDF; relevance; reader growth; protected restore; restart | RUNTIME-ONLY | Static docs identify required experiments and issue no readiness. | Implement and execute; record raw results with owning issue or PR. | Not statically knowable. |
| A021 | Compiler baseline | Ubuntu Rust 1.98 runs `39867915623`, `39868025680`, `39868205817`, `39868374272`, `39869930525`, `39870086718`, `39872058276` | PARTIAL | A022 run `39872058276` checks/clippies only `search-qdrant-bridge --lib` at `7d92fb8b`; commands/logs verified in R009. Earlier broad daemon check exposed legacy/target-bound errors and is not repaired by library success. | Actual consumers, integration-test compilation, Windows and the daemon remain outside the latest library run. | High observed scope; no whole-product build claim. |
| A022 | Runtime qualification and profile identity | bridge real.rs/connect/qualified gate; lexical code_identifiers.rs, encoding.rs, fixtures.rs and candidate caller searches | PARTIAL | Qdrant gate token removed and library-checked. Lexical still hashes test definitions into profile identity and runs fixture admission under operation budgets; record matching is not observed execution. Concrete replacement rule added to lexical FUNCTIONS. | Remove remaining bridge binding/build/ledger/deadline ceremony through actual callers; remove obsolete lexical activation/numerical route rather than integrate it. Preserve useful splitting examples as tests. | High source findings; caller search is not exhaustive branch proof. |

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

- `crates/search-control-redb/SECURITY_RESTRICTION_JOURNAL.md` was another historical explanation of a removed policy journal; its current rule already belongs to search-access and control FUNCTIONS.
- `crates/search-index-qdrant/search-point-identity/S11_PROFILE.md` contained replacement reasoning, but FUNCTIONS simultaneously froze the old S11 surface. Replacement identity/collision/ambiguous-occurrence rules now live in FUNCTIONS; README is an entrypoint.
- `crates/search-control-redb/DIAGNOSTICS.md` remains because it defines a distinct read-only operation rather than a historical gate or copied state machine.

### R006 — xtask and digest-governance removal

Inspected root manifest, lockfile, aliases, workflows and the complete xtask command/module surface. Every command generated or validated removed accepted-evidence, coverage, package maps, integration bootstrap, source boundaries or the canonical digest ledger.

- `f9275731`: delete xtask, including the 3.57 MB ledger; remove the workspace member and cargo_metadata dependency.
- `9d2fe65a`: regenerate Cargo.lock, removing exclusive packages.
- Run `39868374272`: removed directory/package/dependencies, locked metadata, search-contracts check and strict Clippy under Rust 1.98.
- Temporary verifier removed after success.

The broad daemon check failed on its old composition; A021 retains that evidence rather than hiding it.

### R007 — source-shape test removal

Inspected and removed tests reading Rust as text, enforcing module names/byte ceilings or freezing legacy ownership placement rather than behavior:

- 11 Qdrant bridge module-ownership tests (`07c90c14`);
- 5 evaluation module-ownership tests (`30d1921f`);
- 33 daemon module-ownership tests plus four residual DIRECT/source-mapping documents (`789d4fb4`);
- 17 materializer/unitizer/revision-store ownership tests (`a0de1610`).

Retained CRUD/readback/recovery/IDF/metrics/materialization suites. Run `39869930525` passed locked check and strict Clippy for search-materializer, search-unitizer, search-revision-store, search-qdrant-bridge and search-eval.

### R008 — stale-reference and qualification-remnant sweep

Run `39870086718` searched active Markdown/Rust/Cargo after deletion. Stale xtask, package-status and qualification references were corrected; duplicate TOML_PROFILE_2 was removed; ADR 0005 and the optional Research adapter were distilled.

At that checkpoint Qdrant still required QualifiedGate and lexical issued fixture-backed receipts. Subsequent Qdrant edits existed by `7d92fb8b` but this registry was not updated before the chat interrupted. R009 corrects that stale statement; it does not claim those edits were made again.

### R009 — interrupted-cut recovery and remaining qualification audit (2026-10-23)

Inspected exact source head `7d92fb8b5532b5928bdb90ddd42d5685e5f01d48`:

- `crates/search-index-qdrant/search-qdrant-bridge/src/real.rs`;
- `crates/search-index-qdrant/search-qdrant-bridge/src/real/connect_schema/connect.rs`;
- `crates/search-index-qdrant/search-qdrant-bridge/src/qualified/gate.rs`;
- the bridge README, AGENTS and FUNCTIONS;
- `crates/search-lexical/src/code_identifiers.rs`, especially constructor, preflight and profile_definition;
- `crates/search-lexical/src/code_identifiers/encoding.rs` and `fixtures.rs` definitions/admission path;
- lexical AGENTS/FUNCTIONS, root architecture/implementation, current workflow directory and A022 YAML;
- exact-head Actions run metadata, job steps and complete job log.

Findings and changes:

1. Gate removal is already implemented: connect takes no QualifiedGate; report validation returns `()`. Auth binding, exact-build admission, relative timeouts, receipt map and sparse-only path remain. Corrected bridge inventory and this ledger.
2. Bridge README still ordered mutable vector/payload operations and rejected all empty scope; current FUNCTIONS forbids published-state mutation and distinguishes known empty from unknown. README/AGENTS now point to one recipe instead of carrying another algorithm. A001 remains PARTIAL for other surviving documents.
3. Lexical profile identity includes fixture definitions and resource ceilings. Runtime fixture admission also consumes the operation budget. The source-derived 16-byte-input-budget counterexample and the removal boundary are recorded once in [lexical FUNCTIONS](../../crates/search-lexical/FUNCTIONS.md#test-data-must-not-determine-runtime-activation-or-retrieval-identity). No lexical source change or runtime reproduction is claimed.
4. `qualify_code_identifiers_profile` / `encode_code_document` caller searches returned default-branch definitions, reexports and fixtures. Selected source files were read at the current SHA. This does not establish an exhaustive absence of callers in every branch/feature.

Verified historical execution, not a new run:

- [Run 39872058276](https://github.com/UnknownAlienHuman/eliot-search/actions/runs/39872058276), job `120148813047`, 2026-10-22, Ubuntu 24.04, Rust 1.98.0.
- Checkout log confirms `7d92fb8b5532b5928bdb90ddd42d5685e5f01d48`.
- Passed `cargo +1.98.0 check -p search-qdrant-bridge --lib --locked`.
- Passed `cargo +1.98.0 clippy -p search-qdrant-bridge --lib --locked -- -D warnings`.
- No tests, daemon, reclaimer, Windows, live Qdrant or runtime recovery were executed by that run.

This pass changes documentation and removes the leftover push-triggered `.github/workflows/a022-qdrant-direct-connect.yml`; Rust/Cargo are unchanged from that inspected source. Do not automatically run future pushes to compensate for a missing manual dispatch action. Remaining concrete work is A015 residual behavioral-vs-structural tests, A001 surviving document consistency, and A022 actual lexical/bridge consumer cuts; A014/A016/A020 are not declared complete.
