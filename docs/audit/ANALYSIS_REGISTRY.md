# Analysis registry

This is continuation memory for the refactor audit: inspected revisions/paths, findings, changes, execution
evidence and explicit gaps. It is not a permission gate, package-status database or substitute for owner docs.

**Original public baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Latest inspected source:** `44d7416c46554456f16ccd2b3943657b755304ee` (2026-10-23, R013)  
**Historical qualification archive:** `2f9c9720f3e14799a0f8dfefa00077d2fa6a9887/qualification`  
**Rule:** resume from the actual PR head, inspect intervening commits, and bind every runtime/compiler statement
to exact SHA, command and target. Row counts are not a completion percentage.

## Status vocabulary

- **CHECKED** — the named static surface was inspected for the stated question; not every line/runtime property.
- **PARTIAL** — useful findings exist, but named code/callers/reconciliation remain.
- **PENDING** — not inspected enough to direct a safe change.
- **RUNTIME-ONLY** — implementation/execution is required; prose cannot establish it.
- **PROTECTED** — unpublished work is not inspected or overwritten; consume its accepted public handoff.

## Coverage ledger

| ID | Area | Inspected surface | Status | Current conclusion | Remaining |
|---|---|---|---|---|---|
| A001 | Documentation authority/consistency | Root authority; architecture/implementation; current source, preparation, Qdrant, publication, query/window/evidence/protocol/CLI owners | PARTIAL | Historical competing specifications removed. R009-R013 repeatedly found contradictions in the replacement docs and moved each algorithm back to one owner. | Continue surviving owner/issue sweep as concrete code areas are inspected. |
| A002 | Historical governance data | swarm, generated maps, packets/handoffs, context/ticket artifacts | CHECKED | Removed from current tree; Git history is archive. | Code remnants tracked elsewhere. |
| A003 | Workflows/wrappers | `.github/workflows`, `tools/*.ps1`, temporary A022 verifier | CHECKED | Old gates/wrappers removed; Actions remain manual-only. | Recheck only when workflows change. |
| A004 | Workspace inventory | Root manifest and 51 current package manifests | CHECKED | Every member has a receiving/removal/optional/conditional disposition; no target package count. | Physical product-package cutovers. |
| A005 | Entrypoints/runnable product | Daemon/CLI entrypoints, one-shot scan, source sync, preparation CLI, DIRECT runtime/client | PARTIAL | One-shot scan works as a narrow path. Source sync still refuses; target source/Qdrant/query path remains disconnected. R013 confirms active client/server still use sequential line protocol and legacy handles. | Connect actual source → publication → query → CLI and execute it. |
| A006 | Root/config/control | Root owner/config; old journal/snapshot/registry restore; redb 2.6.3 snapshot source | PARTIAL | One root lock and typed current state selected. Full-build input capture must be durable in unserved target tables; an in-process read transaction is not restart state. | Implement/execute capture, commit, Windows lock/durability. |
| A007 | Source acquisition/registry/reconcile | Admission stack; comparator; root currentness/catalog/sync; ignore donor | PARTIAL | R010 found wrong-root absence, dropped compound update, physical/logical identity mismatch, erased reasons and false currentness. Instructions corrected; Rust remains unfixed. | Real source-owner/sync cutover, accepted #598 handoff, focused cases. |
| A008 | Private storage/key overlap | Public #598 contract only | PROTECTED | Obsolete six-domain/purge/compatibility programme is not inherited. | Consume accepted handoff; do not inspect/reset/duplicate private work. |
| A009 | Preparation/identity | Materializer profile/request/pipeline/read/model; unitizer/point identity; old contract | PARTIAL | Golden/limit data contaminates profile identity; execution budget can be tighter than prevalidated request; empty input still refused. Reuse, installation guard and retry eligibility are separate facts. | Implement current preparation and verify budget/empty/ABA/stale completion. |
| A010 | Qdrant/donors | Qdrant bridge, 1.19 mutation/readback/BM25/tokenizer/scoped-IDF; gate/connect | CHECKED for named static questions | QualifiedGate removed. Native BM25 exact-name and scoped-IDF limits documented; real product connection and late-write handling remain. | Connect runtime; execute relevance/scale/late-write cases. |
| A011 | Publication/view/cleanup | Competing publication machines; redb view; late writers; cleanup/restore | PARTIAL | Two-sided desired/published reconciliation, immutable retrieval state and restartable build-input capture selected. Readback is not writer terminality. | Implement crash/late-write/view/cleanup/restore behavior. |
| A012 | Query/evidence/exact/window | Query stack; validator/projector; exact helper; continuation and result-handle catalogues; R012/R013 source paths | PARTIAL | R012 separated mapped coordinates/request stop from item gaps. R013 found full current-catalog scans, global invalidation, latest-source rechecks, arbitrary raw expansion, per-page handle TTL renewal and separate handle/window lifetimes. | Implement one pinned `QueryWindow` and real evidence reader; remove both catalogues/handles. |
| A013 | Protocol/client/CLI | Provider/line stacks; DIRECT session/runtime/output/client; protocol and CLI owners | PARTIAL | R013 confirms no active Cancel reader, multi-line results displayed before terminal, per-line not aggregate output bounds, manual allocation before limit, post-spawn cleanup gaps and unbounded normal wait. | One framed terminal, serviced control reader and owned-child guard; platform execution. |
| A014 | Product depth | Old orientation/comparison/document requirements; enrichers/resolvers/workers/overlay | PARTIAL | User behaviors retained; deleted scaffolds are not delivered capabilities. | Select concrete producers/consumers and assess quality. |
| A015 | Tests/governance code | xtask/ledger/workspace/lock; inspected source-shape tests | PARTIAL | xtask/ledger/exclusive deps removed; 11 Qdrant, 5 evaluation, 33 daemon and 17 prep/store structural tests removed while behavior suites remained. | Find residual structural assertions without deleting useful behavior tests. |
| A016 | Donors | redb, Qdrant, Aho-Corasick, Git/ripgrep/ignore, stable Vec contract | PARTIAL | Donor behavior is read at concrete seams only. `Vec::truncate` retains capacity; redb read snapshots are process-held; ignore has attached errors/depth-zero exceptions. | Revisit only for selected implementation questions. |
| A017 | Qualification tree | Complete tree and representative reports/consumers | CHECKED | Archived in Git; current tree retains only policy/blocker summary. | Runtime approval remnants under A022. |
| A018 | Auxiliary docs | Package/binary root filename sweep, standard/nonstandard supplements | CHECKED for filename/disposition scope | Wave/profile/DIRECT/duplicate helper specs removed; distinct diagnostics retained. | Semantic consistency remains A001. |
| A019 | Issues/PR handoff | #352/#214/#525 and affected product issues through R013 | PARTIAL | Named issues are being replaced with branch-owner links and current findings, not pinned duplicate algorithms. | Continue issue sweep when an owner changes; no whole-queue claim. |
| A020 | Runtime evidence | Windows, blocked I/O, Qdrant late writes/IDF, relevance/scale, reader growth, restart/restore, idle windows/child cleanup | RUNTIME-ONLY | Static counterexamples define tests; they do not establish readiness or reproduced failures. | Implement and run with exact SHA/commands/raw output. |
| A021 | Compiler baseline | Ubuntu Rust 1.98 runs 39867915623, 39868025680, 39868205817, 39868374272, 39869930525, 39870086718, 39872058276 | PARTIAL | Latest verified run covers `search-qdrant-bridge --lib` at `7d92fb8b`; earlier broad daemon check failed. R009-R013 are documentation-only. | Actual consumers, test compilation, Windows and daemon. |
| A022 | Runtime qualification/profile identity | Bridge gate; lexical fixtures/identity; analogous materializer coupling | PARTIAL | Qdrant activation token removed; lexical fixture-backed activation and profile/test coupling remain. | Remove ceremony through real callers; keep causal examples as tests. |

## Pass log

### R001-R005 — remove parallel authority and duplicate local specifications

Removed Architecture 8.4 master, waves, swarm/status/coverage/receipt data, generated maps, wrapper tools and
package-local W7/W8/W10/P18/journal/profile copies. Corrected local-object GC from online gate machinery to
stopped-root maintenance. Key commits through `140852e4`; exact history remains in Git.

### R006 — remove xtask/digest governance

Inspected all xtask command/module families, manifests, aliases and workflows. Removed xtask, its 3.57-MB digest
ledger, workspace member and `cargo_metadata`; regenerated lockfile. Run `39868374272` passed recorded metadata,
`search-contracts` check and strict Clippy under Rust 1.98. Broad daemon check failed and remains recorded.

### R007-R008 — remove source-shape tests and stale qualification references

Removed inspected tests that read Rust/file layout instead of behavior: 11 Qdrant, 5 evaluation, 33 daemon and
17 preparation/store tests. Retained CRUD/readback/recovery/IDF/materialization behavior suites. Run
`39869930525` passed locked check/strict Clippy for the named libraries. R008 corrected stale xtask/status/
qualification references and identified remaining Qdrant/lexical runtime qualification.

### R009 — Qdrant cutover evidence and lexical identity

At `7d92fb8b`, inspected bridge real/connect/gate and lexical identifier profile/fixtures. Confirmed gate removal;
found lexical fixture/resource data in retrieval identity and runtime admission. Corrected owner docs. Historical
run `39872058276` passed check/strict Clippy only for `search-qdrant-bridge --lib`; no tests/daemon/Qdrant runtime.

### R010 — source observation and currentness

At `c0fcf294`, inspected source reconcile comparator, root currentness/catalog/sync and ignore donor source.
Found wrong-root Missing branch, dropped simultaneous content update, physical/logical identity mismatch, erased
causes, hints that leave currentness true, directory probes consuming content hints, positional queue identity,
filtered omission/ignore error/depth-zero bypass gaps. Corrected owner docs and #271/#239; no Rust/run.

### R011 — preparation identity and publication convergence

At `7c7fd36c`, inspected materializer profile/request/pipeline/read/model, daemon preparation CLI, publication/
control/registry/access docs and pinned redb 2.6.3 source. Found test/limit profile coupling, preflight/execution
budget gap, ABA guard conflation, success-only publication comparison and non-durable full-build selection.
Corrected owners and #270/#331/#261; no Rust/run.

### R012 — window admission and evidence failure domains

At `e5350809`, inspected continuation catalogue/prepared output/session/protocol line reader, DIRECT model/spine,
validator through `validate`, query service and old continuation requirements. Found admission after result
allocation, count-only retention, lazy expiry, reader errors flattened into item gaps and raw-only coordinate
assumptions. Preserved prepared-output commit and honest partial completeness checks. Corrected window/evidence
owners and #275/#293 in `44d7416c`; no Rust/run.

### R013 — current-source fences, item handles and client transport (2026-10-23)

Inspected exact head `44d7416c46554456f16ccd2b3943657b755304ee`:

- daemon `result_handles/kernel/{catalog,model,spec,expand}.rs` and `catalog/prepared.rs` in full;
- daemon `source_fence.rs` in full;
- daemon `public_runtime_service/kernel/{runtime,query,session,output_deadline}.rs` in full or previously full
  under R012;
- daemon `protocol_io.rs`, `service_output/kernel/{codec,page}.rs` and query page emission path;
- CLI `public_client.rs` lines 1-760, including spawn/session/drain/parser/shutdown;
- current protocol/CLI/window/evidence owner docs and issues #275/#279/#280/#293/#332.

Confirmed:

1. Handle mint and expansion both enumerate current sources; `source_fence` serializes the complete catalogue
   into a new buffer. Any unrelated current-source change invalidates old tokens, contradicting pinned-view
   semantics and adding corpus-dependent page work.
2. Expansion rechecks latest source active/revision and permits arbitrary raw offsets across the revision.
   Handle records do not retain emitted representation/range. Old published evidence and transformed text cannot
   be represented correctly through this API.
3. Per-page handles receive a new independent TTL and separate commit from the continuation. The target needs
   one fixed-expiry window with item indexes, not a transaction between two obsolete catalogues.
4. Page output is multiple flushed lines; the client prints each before terminal. Partial output can therefore
   become user-visible even when the server commits no cursor/handles. `MAX_RESPONSE_BYTES` is per line, not the
   whole page, and full strings are allocated before that check.
5. The server's inspected loop executes before reading again; its command cancellation `AtomicBool` has no
   active control-reader setter. The client also blocks in `drain_until_terminal`, so Ctrl-C/user Cancel is not
   carried through this session path.
6. `public_client::run_session` has multiple post-spawn `?` exits outside the one explicit terminate branch and
   finishes normal shutdown with an unbounded `wait`. One ordinary owned-child guard must close/terminate/reap
   on every path; no daemon-manager framework is needed.

Corrections live in the window, evidence, protocol and CLI owner docs. Issue bodies are reconciled separately.
Static code/design findings only: no Rust/Cargo/workflow/private #598 changes, build, Clippy, process test or
platform cancellation run. Full current-code caller absence is not claimed. Remaining implementation checks are
listed in the owner recipes and A012/A013/A020.
