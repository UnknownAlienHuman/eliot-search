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
| A001 | Documentation authority | `AGENTS.md`; architecture; implementation; docs/handoff and audit entrypoints | CHECKED | One authority chain selected. Old architecture master, packets, status graph, handoffs and generated maps removed. | Recheck links whenever owner files move. | High static. |
| A002 | Historical governance data | `swarm/**`; generated maps; packet/handoff trees; context/ticket artifacts | CHECKED | Current-tree copies removed; Git history is the archive. | Rust/tool consumers remain under A015. | High. |
| A003 | Workflows and wrappers | `.github/workflows/**`; `tools/*.ps1`; tools README | CHECKED | Wave/map/evidence/bootstrap/source-shape workflows and wrappers removed; one raw manual Cargo workflow remains. | Remove code/tests that still expect retired outputs. | High files, medium consumers. |
| A004 | Workspace inventory | root Cargo manifest; 52 package manifests; workspace disposition | CHECKED | Every member has a receiving/removal disposition; not a target package count. | Physical Cargo/Rust cutovers. | High inventory. |
| A005 | Entrypoints/runnable product | daemon/CLI entry composition; command interceptors; quickstarts | CHECKED | Only bounded one-shot scan is a simple runnable path; persistent product path remains disconnected. | Recheck and execute after direct dispatcher. | High static. |
| A006 | Root/config/control | runtime owner; config; generic control journal/snapshot; registry restore | CHECKED | One root lock, typed current rows and finite unfinished external work selected. | Compile and test Windows lock/durability. | High static. |
| A007 | Source acquisition/registry | admission, receipts, provenance, records/history/registry/reconcile, safe reader, daemon source path | CHECKED | Exact acquired bytes must feed one object writer and current head; registry replay/original-input archive are removal targets. | Integrate accepted #598 handoff and crash cases. | High static. |
| A008 | Private storage/key overlap | public #598 issue contract and public-main boundaries only | PROTECTED | Obsolete six-domain/purge/compatibility programme is not inherited. | Consume only accepted public handoff; do not inspect/reset/duplicate private work. | High boundary, implementation unknown. |
| A009 | Preparation/identity | materializer, unitizer, empty/gap cases, projection and point identity; current S11 implementation/profile docs | CHECKED | One materialization, recoverable input/outcome comparison, stale-result guard, conservative occurrence reuse and versioned evidence selected. Point identity owner now states the replacement directly; revision-bound S11 is not compatibility. | Implement and measure rewrite amplification and duplicate edits. | High static. |
| A010 | Qdrant/donor behavior | real bridge callers; Qdrant 1.19 mutation/readback/BM25/tokenizer/scoped-IDF source | CHECKED | Real adapter disconnected; qualification credential and process receipt ledger removed from target. IDF may scan broad population; exact identifiers need a separate plane. | Delete qualification-bound bridge code; run live late-write/relevance/scale cases. | High inspected source. |
| A011 | Publication/view/cleanup | competing publication machines; control intents; epoch pins; reclaimer; retention; restore | CHECKED | One pending/current/unfinished-cleanup owner selected. Reader drain and possible late writers both constrain Qdrant deletion. Local object GC is stopped-root maintenance. | Implement delayed-RPC, reader-growth, backlog, GC and restore/import cases. | High static. |
| A012 | Query/evidence/exact | planner/executor/access/validator/projector/handles/continuations; literal matcher; oracle path | CHECKED | Direct handlers, one request context, retained evidence and explicit exact input semantics selected. | Implement real Qdrant/evidence path and streaming cancellation. | High static. |
| A013 | Protocol/client/CLI | provider protocol/client; daemon provider/public runtime; CLI legacy/typed stacks | CHECKED | One framed live session selected; dead locators, blocked-I/O cancellation, partial output and cursor accounting contradictions corrected. | Implement and execute same-session Continue/Expand/cancel/child cleanup. | High static. |
| A014 | Product depth | old orientation/comparison/document requirements; enrichers/resolvers/comparator/workers/overlay | PARTIAL | Observable user behavior preserved; scaffolds are not capabilities. | Select concrete producers/consumers and evaluate quality. | Medium. |
| A015 | Tests/governance code | `xtask/**`; module/source-shape tests; harness inventory; qualification consumers | PARTIAL | Generated authority and wrappers mapped for deletion. Qualification consumers are retired validators/tests, not runtime authority. | Delete xtask, digest ledger, exclusive fixtures and Cargo consumers; run minimal Clippy. | High inventory, medium closure. |
| A016 | Donors | redb 2.6.3; Qdrant 1.19; Aho-Corasick; selected Git/ripgrep/Tantivy/Lucene mechanics | PARTIAL | Mechanisms and hidden costs separated from product claims; API catalogues are not implementation checklists. | Revisit only for concrete owner decisions. | Medium-high named mechanisms. |
| A017 | Qualification tree | Complete tree inventory; root policy/blockers; Qdrant/code-enrichment/os-secret samples; consumer searches | CHECKED | Old W1-W10/gate/PASS/FAIL database archived at `2f9c9720`; current tree retains only policy and blocker summary. | Remove qualification-bound Rust/tests under A010/A015. | High disposition. |
| A018 | Package-local auxiliary docs | Complete current-tree package/binary root sweep; xtask supplement-name set; nonstandard journal/profile/helper files | CHECKED | All W7/W8/W10/P18 supplements, copied window helpers, old control journal/checkpoint/quarantine notes, `SECURITY_RESTRICTION_JOURNAL.md` and duplicate `S11_PROFILE.md` were inspected and removed. Unique current `DIAGNOSTICS.md` remains. Point identity rules moved into its sole `FUNCTIONS.md` owner. | Recheck only when a package adds or renames an auxiliary owner document. Rust references to removed names are A015. | High static. |
| A019 | Open issues | Current implementation chain and obvious duplicate/superseded tasks; #352 | CHECKED | Issues reconciled to current owners; old harness/graph/lint/status tasks closed. | Refresh after material code/issue changes. | Medium-high. |
| A020 | Runtime evidence | Windows durability/cancellation; live Qdrant late writes/IDF; relevance; reader growth; protected restore; restart | RUNTIME-ONLY | Static docs identify required experiments and issue no readiness. | Implement and execute; record raw results with owning issue/PR. | Not statically knowable. |

## Pass log

### R001 — authority/governance removal

Removed the Architecture 8.4 master, wave/handoff/status/swarm/generated authority and redundant workflow/tool surfaces through `a4af41f8`. Historical evidence remains in Git.

### R002 — first package-local wave cleanup

Inspected and removed CLI/provider W8, query W7, query-window helper, Qdrant bridge P18 and control journal/cutover/intent/quarantine/checkpoint copies. They either prescribed removed mechanisms or repeated current owner rules. Commit: `2f9c9720`.

### R003 — qualification archive disposition

Inspected the complete qualification tree, representative raw reports and code consumers. Replaced the current tree with a policy and current blocker summary; exact archive remains at `2f9c9720`. Commit: `49f5c46b`.

### R004 — standard supplement closure and GC contradiction

Inspected the supplement naming policy in `xtask/src/coverage_graph_generation/derive/operations.rs` and every standard supplement remaining on the current branch. The daemon W8/W10 files still ordered grants, accepted receipts, capability snapshots, worker qualification, quarantine and migration/rollback state. The others copied current publication/view/cleanup/evaluation summaries.

Owner verification found a replacement error in `search-index-reclaimer/FUNCTIONS.md`: its local artifact section still prescribed online shared/exclusive GC while current implementation selected explicit stopped-root maintenance. The owner map was rewritten: publication owns Qdrant cleanup; local object collection runs only with daemon/producers/windows stopped and one exclusive root lock. Standard supplements were removed in `140852e4`.

### R005 — nonstandard package-local closure

Inspected current package/binary roots rather than default-branch search results. Checked arbitrary uppercase/current-looking documents including journal, checkpoint, quarantine, recovery, profile, integration, scale, security and snapshot names.

Findings:

- `crates/search-control-redb/SECURITY_RESTRICTION_JOURNAL.md` was another historical explanation of a removed policy journal; its current rule already belongs to `search-access` and control `FUNCTIONS.md`.
- `crates/search-index-qdrant/search-point-identity/S11_PROFILE.md` contained useful replacement reasoning, but `FUNCTIONS.md` simultaneously froze the old S11 public surface. That was two conflicting owners. The replacement identity, collision rule, ambiguous-occurrence policy and cutover now live directly in `FUNCTIONS.md`; README is only an entrypoint.
- `crates/search-control-redb/DIAGNOSTICS.md` is retained because it defines a distinct current read-only operation rather than a historical gate or copied state machine.

Action: remove the redundant journal/profile files and mark A018 checked. The next static/code pass is A015: remove `xtask`, digest/shape/qualification consumers and obsolete Cargo wiring, then run minimal compiler/Clippy for the changed tooling surface.
