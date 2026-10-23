# Analysis registry

This file records what the refactor audit actually inspected, what changed and what remains. It is a memory aid, not an approval gate, package-status system or substitute for the owning implementation documents.

**Original public source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Latest inspected source revision:** `c0fcf2947b0ce6118ae67ee94e557632bd2a809d` (2026-10-23, R010).  
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
| A001 | Documentation authority and consistency | AGENTS; architecture/implementation; handoff/audit entrypoints; bridge and source-reconcile README/AGENTS/FUNCTIONS | PARTIAL | R009 removed competing bridge recipes. R010 found the newer reconcile recipe still understated comparator defects and filtered-inventory semantics; corrected its owner, linked identity/preparation rules and reduced local instruction duplication. | Reconcile other surviving current owner documents. File removal is not semantic consistency. | High for named findings, incomplete cross-owner sweep. |
| A002 | Historical governance data | `swarm/**`; generated maps; packet/handoff trees; context/ticket artifacts | CHECKED | Current-tree copies removed; Git history is the archive. | Rust/tool consumers are tracked under A015/A022. | High. |
| A003 | Workflows and wrappers | `.github/workflows/**`; `tools/*.ps1`; tools README; A022 workflow and run | CHECKED | Wave/map/evidence/bootstrap/source-shape workflows removed. R009 removed the leftover push-triggered A022 verifier; one manual raw Cargo diagnostic remains. | Keep future checks explicit; do not leave automatic temporary workflows after interruption. | High for inspected tree. |
| A004 | Workspace inventory | root Cargo manifest; 51 current package manifests; workspace disposition | CHECKED | Every current member has a receiving, removal, optional or conditional disposition; this is not a target package count. xtask is no longer a member. | Physical product-package cutovers. | High inventory. |
| A005 | Entrypoints/runnable product | daemon/CLI entry composition; command interceptors; quickstarts; source_sync rechecked in R010 | CHECKED for named routes | Only bounded one-shot scan is a simple runnable path. Source sync still returns SOURCE_ROOT_SYNC_EFFECTS_UNAVAILABLE; persistent product path remains disconnected. | Recheck and execute after direct dispatcher and source integration. | High static, not an executed product. |
| A006 | Root/config/control | runtime owner; config; generic control journal/snapshot; registry restore | CHECKED | One root lock, typed current rows and finite unfinished external work selected. | Compile and test Windows lock/durability. | High static. |
| A007 | Source acquisition/registry/reconciliation | admission/receipt/provenance/records/history/registry/safe-reader paths; R010 exact comparator, root_currentness, catalog refresh and sync entrypoint | PARTIAL | The ingestion ownership diagnosis remains useful, but R010 found wrong-root absence, omitted compound content update, physical/logical identity mismatch, erased reasons, false currentness and positional hints. New instructions now distinguish filtered omission, explicit exclusion, confirmed absence and incomplete observation. Rust defects are NOT fixed by these doc edits. | Replace comparison/application and real sync together; integrate accepted #598 handoff. Exercise R010 cases after implementation. Do not mark source semantics closed from inventory alone. | High inspected branches; connected behavior unimplemented/unrun. |
| A008 | Private storage/key overlap | public #598 issue contract and public-main boundaries only | PROTECTED | Obsolete six-domain, purge and compatibility programme is not inherited. | Consume only accepted public handoff; do not inspect, reset or duplicate private work. | High boundary, implementation unknown. |
| A009 | Preparation/identity | materializer/unitizer/projection/point identity; S11 cutover; logical-source matching rechecked in R010 | CHECKED for named static questions | Recoverable input/outcome comparison, stale-result guard and conservative occurrence reuse selected. R010 corrects the reconcile instructions: same-path atomic save need not create a new logical source; same bytes after rename do not guarantee unchanged transform context. | Implement and measure rewrite amplification/duplicate edits; handle processing-policy changes without invalidating unchanged successful retrieval inputs. | High static, not all preparation behavior. |
| A010 | Qdrant/donor behavior | bridge callers; Qdrant 1.19 mutation/readback/BM25/tokenizer/scoped-IDF; real.rs/connect/gate | CHECKED for named static questions | Real adapter remains disconnected from normal product flow. QualifiedGate input removed at 7d92fb8b; fixed build/auth/deadline/ledger machinery remains. IDF cost and exact identifier limits remain explicit. | Complete direct runtime and execute late-write/relevance/scale cases. | High inspected source; not product execution. |
| A011 | Publication/view/cleanup | competing publication machines; control intents; epoch pins; reclaimer; retention; restore | CHECKED | One pending/current/unfinished-cleanup owner selected. Readers and possible late writers constrain Qdrant deletion. Local object GC is stopped-root maintenance. | Implement delayed-RPC, reader-growth, backlog, GC and restore/import cases. | High static. |
| A012 | Query/evidence/exact | planner/executor/access/validator/projector/handles/continuations; literal matcher; oracle path | CHECKED | Direct handlers, one request context, retained evidence and explicit exact input semantics selected. | Implement real Qdrant/evidence path and streaming cancellation. | High static. |
| A013 | Protocol/client/CLI | provider protocol/client; daemon provider/public runtime; CLI legacy/typed stacks | CHECKED | One framed live session selected; dead locators, blocked-I/O cancellation, partial output and cursor-accounting contradictions corrected. | Implement and execute same-session Continue/Expand/cancel/child cleanup. | High static. |
| A014 | Product depth | old orientation/comparison/document requirements; enrichers/resolvers/comparator/workers/overlay | PARTIAL | Observable user behavior preserved; scaffolds are not capabilities. | Select concrete producers/consumers and evaluate quality. | Medium. |
| A015 | Tests/governance code | xtask; workspace/lock/workflows; product ownership/source-text tests | PARTIAL | xtask, 3.57 MB ledger and exclusive lock dependencies removed. Removed 11 Qdrant, 5 evaluation, 33 daemon and 17 preparation/store source-shape tests; behavior suites retained. Runs 39868374272, 39869930525 and 39870086718 passed their recorded checks. | Find residual source/file/document assertions without behavioral coverage. No additional test deletion or compiler run in R010. | High removed surfaces, incomplete residual search. |
| A016 | Donors | redb 2.6.3; Qdrant 1.19; Aho-Corasick; selected Git/ripgrep/Tantivy/Lucene mechanics; R010 ignore walk source at 4649aa9 and published 0.4.33 docs | PARTIAL | R010 verified attached entry errors, partial ignore parsing, depth-zero filtering exception and metadata-size omission. Dependency reuse does not supply Eliot's inventory/selection semantics automatically. | Revisit concrete decisions only; no watcher/version upgrade or whole-donor audit claimed. | High named source branches; other donor questions remain. |
| A017 | Qualification tree | Complete tree inventory; root policy/blockers; Qdrant/code-enrichment/os-secret samples; consumer searches | CHECKED | Old W1-W10/gate/PASS/FAIL database archived at 2f9c9720; current tree retains policy and blocker summary only. | Rust approval semantics remain A022. | High disposition. |
| A018 | Package-local auxiliary filenames | Current package/binary root sweep; standard and nonstandard helper files | CHECKED for filename/disposition scope | W7/W8/W10/P18, copied window/control/profile and daemon DIRECT/source-mapping notes, and duplicate TOML specification removed. Unique current DIAGNOSTICS remains. | Surviving-document semantics are A001, not established by this row. | High stated scope. |
| A019 | Open issues/PR handoff | Current chain; #352/#214/#525; PR #610; #263 in R009; #271/#239 in R010 | PARTIAL | Superseded harness/graph/lint/status tasks closed; #525 complete, #214 open. R010 reconciles optional watcher versus required inventory, stale pinned recipes and unconditional rename reuse. | Refresh changed owners in current issues. No whole-queue reread claimed. | High named issues, incomplete queue. |
| A020 | Runtime evidence | Windows durability/cancellation; Qdrant late writes/IDF; relevance; reader growth; protected restore; restart; inventory/event cases | RUNTIME-ONLY | Static counterexamples identify execution needs, not readiness. | Implement and execute with exact SHA/commands and raw outcome. | Not statically established. |
| A021 | Compiler baseline | Ubuntu Rust 1.98 runs 39867915623, 39868025680, 39868205817, 39868374272, 39869930525, 39870086718, 39872058276 | PARTIAL | Latest verified A022 run checks/clippies search-qdrant-bridge --lib at 7d92fb8b only. Earlier broad daemon check failed on old composition. R010 changes no Rust/Cargo and claims no new run. | Actual consumers, integration-test compilation, Windows and daemon are outside that library result. | High observed scope; no whole-product build claim. |
| A022 | Runtime qualification and profile identity | bridge real.rs/connect/qualified gate; lexical code_identifiers.rs, encoding.rs, fixtures.rs and candidate caller searches | PARTIAL | Gate token removed/library-checked. Lexical still hashes tests into profile identity and runs fixture admission under operation budgets. Replacement rule is in lexical FUNCTIONS. | Remove remaining bridge ceremony and obsolete lexical activation/numerical route through real callers. Preserve causal examples as tests. | High named findings; not exhaustive caller proof. |

## Pass log

### R001 — authority/governance removal

Removed Architecture 8.4 master, wave/handoff/status/swarm/generated authority and redundant workflow/tool surfaces through `a4af41f8`. Historical evidence remains in Git.

### R002 — first package-local wave cleanup

Inspected and removed CLI/provider W8, query W7, query-window helpers, Qdrant bridge P18 and control journal/cutover/intent/quarantine/checkpoint copies. They prescribed removed mechanisms or repeated current owner rules. Commit: `2f9c9720`.

### R003 — qualification archive disposition

Inspected the complete tree, representative reports and code consumers. Retained policy/current blockers; exact archive at `2f9c9720`. Commit: `49f5c46b`.

### R004 — standard supplement closure and GC contradiction

Inspected the old xtask supplement naming policy and remaining standard supplements. Daemon W8/W10 still ordered grants, accepted receipts, capability snapshots, worker qualification, quarantine and migration state; other files copied current owner summaries.

`search-index-reclaimer/FUNCTIONS.md` still prescribed online shared/exclusive local GC while implementation selected stopped-root maintenance. Corrected the owner distinction and removed standard supplements in `140852e4`.

### R005 — nonstandard package-local closure

Inspected current package/binary roots, not default-branch search results. Checked journal/checkpoint/quarantine/recovery/profile/integration/scale/security/snapshot names.

- Removed `search-control-redb/SECURITY_RESTRICTION_JOURNAL.md`; current rules belong to access/control owners.
- `search-point-identity/S11_PROFILE.md` explained replacement while FUNCTIONS froze old S11. Moved replacement/collision/ambiguous-occurrence rules into FUNCTIONS; removed the duplicate profile and reduced README.
- Retained `search-control-redb/DIAGNOSTICS.md`: a distinct read-only operation, not a copied state machine.

### R006 — xtask and digest-governance removal

Inspected manifest/lock/aliases/workflows and the complete xtask command/module surface. Every command served removed evidence/coverage/package-map/bootstrap/source-boundary/digest governance.

- `f9275731`: delete xtask, 3.57 MB ledger, workspace member and cargo_metadata dependency.
- `9d2fe65a`: regenerate Cargo.lock, removing exclusive packages.
- Run `39868374272`: removed directory/package/dependencies, locked metadata, search-contracts check and strict Clippy under Rust 1.98.
- Temporary verifier removed. The broad daemon check failed on old composition; A021 retains that limitation.

### R007 — source-shape test removal

Removed inspected tests reading Rust as text, enforcing module names/byte ceilings or freezing legacy placement:

- 11 Qdrant bridge ownership tests (`07c90c14`);
- 5 evaluation ownership tests (`30d1921f`);
- 33 daemon ownership tests and four residual DIRECT/source-mapping docs (`789d4fb4`);
- 17 materializer/unitizer/revision-store ownership tests (`a0de1610`).

CRUD/readback/recovery/IDF/metrics/materialization suites retained. Run `39869930525` passed locked check and strict Clippy for search-materializer, search-unitizer, search-revision-store, search-qdrant-bridge and search-eval.

### R008 — stale-reference and qualification-remnant sweep

Run `39870086718` searched active Markdown/Rust/Cargo. Corrected stale xtask/package-status/qualification references; removed duplicate TOML_PROFILE_2; distilled ADR 0005 and optional Research adapter.

At that checkpoint Qdrant still required QualifiedGate and lexical issued fixture-backed receipts. Subsequent Qdrant edits were present by `7d92fb8b` but the registry lagged after interruption. R009 corrected the stale statement, not repeated the code edit.

### R009 — interrupted-cut recovery and qualification audit (2026-10-23)

Inspected at `7d92fb8b5532b5928bdb90ddd42d5685e5f01d48`:

- bridge `src/real.rs`, `src/real/connect_schema/connect.rs`, `src/qualified/gate.rs` and README/AGENTS/FUNCTIONS;
- lexical `src/code_identifiers.rs` constructor/preflight/profile_definition and `code_identifiers/{encoding,fixtures}.rs`, plus lexical AGENTS/FUNCTIONS;
- root architecture/implementation, workflow directory/A022 YAML, exact-head Actions metadata/job/log.

Gate removal was implemented: connect takes no QualifiedGate; report validation returns `()`. Auth/fixed-build/timeouts/ledger and sparse-only paths remain. Corrected the inventory and contradictory bridge README/AGENTS mutation/empty-scope rules.

Lexical hashes fixture definitions/resource ceilings into profile identity and runs fixture admission under the operation budget. The 16-byte-input-budget counterexample and replacement rule live in [lexical FUNCTIONS](../../crates/search-lexical/FUNCTIONS.md#test-data-must-not-determine-runtime-activation-or-retrieval-identity). Lexical source was not changed or executed. Caller search returned default-branch definitions/reexports/fixtures, not exhaustive branch/feature absence.

Verified historical run, not a new execution:

- [39872058276](https://github.com/UnknownAlienHuman/eliot-search/actions/runs/39872058276), job `120148813047`, 2026-10-22, Ubuntu 24.04, Rust 1.98.0; checkout `7d92fb8b5532b5928bdb90ddd42d5685e5f01d48`.
- Passed `cargo +1.98.0 check -p search-qdrant-bridge --lib --locked` and `cargo +1.98.0 clippy -p search-qdrant-bridge --lib --locked -- -D warnings`.
- No tests/daemon/reclaimer/Windows/live Qdrant/recovery executed.

Commit `c0fcf294` changed docs and removed the leftover automatic A022 workflow. Do not add automatic pushes to compensate for a missing manual-dispatch tool. A001/A015/A022 and product/donor/runtime questions remained open.

### R010 — source observation, selection and freshness audit (2026-10-23)

Inspected at `c0fcf2947b0ce6118ae67ee94e557632bd2a809d`:

- `crates/search-source/search-source-reconcile/src/lib.rs`, definitions and comparison/error/ledger branches through the unit-test entry;
- `crates/search-source/search-source-records/src/root_currentness.rs`, implementation and relevant existing tests through line 570;
- `bins/eliot-searchd/src/source_roots/kernel/catalog.rs`, load/refresh/registration/hint boundary through line 300;
- `bins/eliot-searchd/src/owner_composition/kernel/source_sync.rs`, full refusal-only sync path;
- source README; reconcile README/AGENTS/FUNCTIONS; admission and identity FUNCTIONS;
- old reconcile FUNCTIONS at public baseline `c8e38b6`; current #271/#239 issue bodies;
- donor `ignore/src/walk.rs` at `4649aa9700619f94cf9c66876e9549d83420e16c`, relevant error/filter branches; published ignore 0.4.33 DirEntry/WalkBuilder docs. This does not update a dependency or re-audit notify/gix.

Confirmed static defects: Missing bypasses the root check; first physical-key binding omits concurrent content change; same-path replacement/no-key cases conflict with current logical-source policy; reason helpers discard causes; pending hints do not invalidate workspace_current; root probes clear content hints; insert/remove shifts positions without rebasing hints. The method sequence available -> synced -> Modified still reports workspace_current in the inspected code.

Replacement-document gaps: filtered omission was still too easily read as absence; builder/traversal/attached-entry errors were not distinguished; a watcher path used as a depth-zero walk root could bypass normal discovery filtering; same-content rename was described as unconditional preparation reuse. Old scope/incomplete-inventory requirements remain useful, not their receipts/platform.

Changes: reconcile FUNCTIONS now owns those distinctions, guarded current-row application and a small take-at-start dirty-latch lifecycle. README/AGENTS point to it. #271/#239 use the current recipe and agree that inventory precedes the optional watcher. No new receipt, event ledger, watcher service or control plane is selected. A007 returns to PARTIAL; prior broad CHECKED was insufficient for these semantics.

Evidence boundary: code-derived counterexamples, NOT runtime reproductions. Sync remains unavailable, and search for reconcile_source returned only its definition on default main; this is not exhaustive absence of current-branch callers. No Rust/Cargo, tests, private #598 code, workflow or build changed/run. Local clone failed DNS; the named source was read through the GitHub connector at the exact SHA. Remaining work is the actual source-owner/sync cutover and case checks in the [owner recipe](../../crates/search-source/search-source-reconcile/FUNCTIONS.md), plus still-open A001/A015/A022 and product/runtime questions.
