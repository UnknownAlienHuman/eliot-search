# Analysis registry

Continuation memory for the refactor audit: exact inspected revisions/paths, findings, changes, execution
evidence and remaining uncertainty. It is not a permission gate, package-status database or substitute for owner
instructions.

**Original public baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Latest inspected source:** `bf2bde299b3f009527397571bd9baeea51fde840` (`R016`, 2026-10-23)  
**Historical qualification archive:** `2f9c9720f3e14799a0f8dfefa00077d2fa6a9887/qualification`  
**Rule:** resume from the actual PR head, inspect intervening commits, and bind every compiler/runtime claim to
an exact SHA, command and target. Row counts are not a completion percentage.

## Status vocabulary

- **CHECKED** — named static surface inspected for the stated question; not every line/runtime property.
- **PARTIAL** — useful findings exist, but named code/callers/reconciliation remain.
- **PENDING** — insufficient inspection to direct a safe change.
- **RUNTIME-ONLY** — implementation/execution required; prose cannot establish it.
- **PROTECTED** — unpublished work is not inspected/overwritten; consume its accepted public handoff.

## Coverage ledger

| ID | Area | Inspected surface | Status | Current conclusion | Remaining |
|---|---|---|---|---|---|
| A001 | Documentation authority/consistency | Root authority; architecture/implementation; current source, preparation, Qdrant, publication, query, protocol, CLI, maintenance and test owners | PARTIAL | Historical parallel specifications were removed. R009-R016 repeatedly found contradictions in replacement documents and moved each algorithm back to one owner. | Continue the surviving owner/issue sweep only with concrete source inspection; do not create another global specification. |
| A002 | Historical governance data | Swarm, generated maps, packets/handoffs, context/ticket artifacts | CHECKED | Removed from the current tree; Git history is the archive. | Code remnants are tracked below. |
| A003 | Workflows/wrappers | Workflows, PowerShell wrappers and temporary qualification verifier | CHECKED | Old gates/wrappers removed; Actions remain manual-only. | Recheck only when workflows change. |
| A004 | Workspace/package inventory | Root manifest and 51 current workspace members | CHECKED for inventory | Every member has a receiving/removal/optional/conditional disposition. Package count itself is not a target. | Physical cutovers and Cargo pruning. |
| A005 | Entrypoints/runnable product | Daemon/CLI interceptor chains, one-shot scan, source sync, preparation CLI, DIRECT runtime/client/maintenance; R016 standalone snapshot/sealed targets | PARTIAL | One-shot scan is a narrow runnable island. R016 removes eight separate snapshot/sealed Cargo applications and the exclusive snapshot implementation. The primary daemon still has an interceptor chain and disconnected source→Qdrant→query path. | One default daemon/CLI application; remove interceptors, compatibility commands, shared dead modules and hot-path diagnostics. |
| A006 | Root/config/control | Root owner/config; old journal/snapshot/registry restore; redb 2.6.3 snapshot; stopped-root/archive boundary | PARTIAL | One root guard and typed current state selected. A process-held read snapshot is not durable build selection; copied locks/external authority are not restore state. | Implement/execute typed state, full-build input capture, Windows lock/durability and archive publication. |
| A007 | Source acquisition/registry/reconciliation | Admission stack; comparator; currentness/catalog/sync; ignore donor | PARTIAL | R010 found wrong-root absence, dropped compound update, physical/logical identity mismatch, erased reasons, positional work and false currentness. Instructions were corrected; Rust remains unfixed. | Real source/object/current-row cutover, accepted #598 handoff and focused cases. |
| A008 | Private storage/key overlap | Public #598 contract only | PROTECTED | The obsolete six-domain/purge/compatibility programme is not inherited. | Consume the accepted handoff; do not inspect/reset/duplicate unpublished work. |
| A009 | Preparation/identity | Materializer profile/request/pipeline/read/model; unitizer/point identity | PARTIAL | Test/resource data contaminates profile identity; execution budget can be tighter than prevalidation; empty input is still refused; reuse, current installation guard and retry eligibility are different facts. | Implement current preparation and verify empty/gap/budget/ABA/stale completion. |
| A010 | Qdrant/donors | Bridge; Qdrant 1.19 mutation/readback/BM25/tokenizer/scoped-IDF; activation gate/connect | CHECKED for named static questions | QualifiedGate was removed. Exact-name/tokenizer, scoped-IDF cost and late-writer limitations are explicit. R016 deletes the separate local BM25 prototype rather than treating it as a fallback. | Connect the real product path and execute relevance/scale/late-write cases. |
| A011 | Publication/view/cleanup | Competing publication machines; redb view; late writers; cleanup and restore boundary | PARTIAL | Two-sided desired/published reconciliation, immutable retrieval state, durable captured build inputs and isolated-generation recovery were selected. Readback is not writer terminality. | Implement crash/view/cleanup/rebuild/restore behavior and execute it. |
| A012 | Query/evidence/window | Query stack, validator/projector, exact helper, continuation and result-handle catalogues | PARTIAL | R012/R013 found post-allocation admission, collapsed failure domains, raw-only coordinates, whole-source fences, latest-source rechecks, arbitrary expansion and split lifetimes. | One pinned `QueryWindow`, direct Qdrant path and real evidence reader; remove both catalogue families. |
| A013 | Protocol/client/CLI | Provider/line stacks, DIRECT session/output/client; R016 old TCP/token snapshot pair | PARTIAL | No active Cancel reader; multi-line partial results become visible before terminal; bounds are per line; manual strings allocate first; child cleanup has escape paths. R016 deletes a second obsolete TCP/token client/server instead of adapting it. | One capped framed terminal, serviced control path, owned-child guard and platform execution. |
| A014 | Product depth | Old orientation/comparison/document requirements; enrichers/resolvers/workers/overlay | PARTIAL | User-visible goals are retained; deleted scaffolds are not capabilities. | Select concrete producers/consumers and assess quality after baseline Search works. |
| A015 | Tests/governance code | xtask/ledger; source-shape removals; R015 lifecycle tests; R016 snapshot target/source test | PARTIAL | xtask/ledger and 66 prior structural tests were removed. R015 removed 15 more structural/lifecycle files. R016 removes `legacy_snapshot_isolation.rs` and the code it existed to freeze. Mixed migration tests still combine real I/O cases with source assertions. | Extract causal cases, remove remaining source-text/process proofs with their owners, then run changed-owner compiler/Clippy. |
| A016 | Donors | redb, Qdrant, Aho-Corasick, Git/ripgrep/ignore and stable Vec behavior | PARTIAL | Donor behavior is read only at concrete seams. No whole-donor or upgrade claim. | Revisit only for a selected implementation question. |
| A017 | Qualification tree | Complete tree and representative reports/consumers | CHECKED | Archived in Git; current tree retains only policy/blocker summary. | Runtime ceremony is A022. |
| A018 | Auxiliary docs | Package/binary root filename sweep and standard/nonstandard supplements | CHECKED for filename/disposition | Duplicate wave/profile/DIRECT specifications removed; distinct diagnostics retained. | Semantic consistency remains A001. |
| A019 | Issues/PR handoff | #352/#214/#525 and affected product issues through R016 | PARTIAL | Named issues link current branch owners instead of stale pinned algorithms. | Update only when an owner or inspected source changes; no whole-queue claim. |
| A020 | Runtime evidence | Windows, blocked I/O, Qdrant late writes/IDF, relevance/scale, reader growth, restart/restore, child/GC/archive | RUNTIME-ONLY | Static counterexamples define required runs; they do not establish readiness or reproduce every failure. | Implement and run with exact SHA/commands/raw outcome. |
| A021 | Compiler baseline | Ubuntu Rust 1.98 runs through `39872058276` | PARTIAL | Latest verified run covers `search-qdrant-bridge --lib` at `7d92fb8b`; an earlier broad daemon check failed. R009-R016 make no new compiler claim. R016 removes only explicit test targets/exclusive source and changes no dependency/lock graph. | Actual consumers, daemon, tests and Windows. |
| A022 | Runtime qualification/profile identity | Qdrant gate; lexical fixture identity; materializer analogue | PARTIAL | Qdrant activation token removed; lexical fixture-backed activation and profile/test coupling remain. | Remove ceremony through real callers; retain causal examples as ordinary tests. |
| A023 | Local GC/storage diagnostics | Legacy maintenance/guard/status and query refresh callers | PARTIAL | Legacy GC keeps every logged revision yet misses current preparation/published/pending/cleanup roots. Query-time status repeats full log/object/preparation walks. | Implement typed stopped-root mark/sweep and remove recursive inventory from query/output. |
| A024 | Backup/restore/import | Generic retention crate; feature-gated restore composition; #243/#244 | PARTIAL | Old restore is same-installation receipt/cutover/key authority. Target is completed archive plus fresh destination/Qdrant generation, with copied pending/cleanup non-executable. | Implement partial-marker, corrupt-copy, interrupted import and protected-storage cases; remove old platform. |
| A025 | Feature/harness topology | Root/daemon/CLI manifests; `wave1`–`wave7`; eight snapshot/sealed targets; their exact source/callers | PARTIAL | R016 removes all eight explicit snapshot/sealed Cargo targets and their exclusive entrypoint/source trees. Feature ladders, optional old-package wiring and shared `sealed_*`/restore/migration modules remain. | Remove wave ladder and shared dead modules with actual current callers; do not create another acceptance profile. |

## Remaining work queue

This is the current ordered backlog, not a gate:

1. **Entrypoint and feature cut:** one daemon/CLI path; remove interceptor chain, wave ladder, optional old-package
   wiring and shared sealed/migration/restore compatibility modules after their last current callers move.
2. **Source/object/current state:** accepted #598 handoff, retained bytes before current-head commit, real sync,
   current-row recovery and preparation scheduler.
3. **Qdrant/publication:** connect one runtime; immutable published retrieval state; pending/readback/late-writer
   handling; atomic view/evidence commit and finite cleanup.
4. **Query/session:** direct scoped BM25, pinned composite evidence lookup, real retained read, one `QueryWindow`,
   framed terminal, active Cancel and owned-child shutdown.
5. **Maintenance/archive/import:** typed stopped-root GC; archive-complete marker last; fresh destination identity and
   Qdrant generation; no copied external authority.
6. **Residual test/qualification cleanup:** mixed migration source assertions, lexical fixture activation,
   disconnected process proofs and obsolete package consumers.
7. **Product depth:** exact/navigation/comparison/documents/MCP only through real producers and consumers.
8. **Runtime evidence:** Windows durability/cancellation, live Qdrant unknown writes/IDF/relevance/scale, restart,
   old-reader growth, GC/archive/import and connected default-binary smoke.

## Pass log

### R001-R005 — remove parallel authority and duplicate specifications

Removed Architecture 8.4 master, waves, swarm/status/coverage/receipt data, generated maps, wrappers and
package-local W7/W8/W10/P18/journal/profile copies. Corrected local GC from online gate machinery to stopped-root
maintenance. Exact history remains in Git.

### R006 — xtask/digest governance

Removed xtask, its 3.57-MB ledger, workspace member and `cargo_metadata`; regenerated the lockfile. Run
`39868374272` passed recorded metadata/`search-contracts`/strict Clippy under Rust 1.98. Broad daemon check failed.

### R007-R008 — source-shape tests and stale qualification

Removed 11 Qdrant, 5 evaluation, 33 daemon and 17 preparation/store topology tests while retaining behavior
suites. Run `39869930525` passed named-library check/strict Clippy. R008 corrected stale references and located
remaining runtime qualification.

### R009 — Qdrant/lexical qualification

At `7d92fb8b`, confirmed Qdrant gate removal; found lexical fixture/resource data in profile identity and runtime
admission. Run `39872058276` passed check/Clippy only for `search-qdrant-bridge --lib`; no daemon/live Qdrant.

### R010 — source observation/currentness

At `c0fcf294`, inspected reconcile/currentness/catalog/sync and ignore source. Found wrong-root absence, compound
update loss, physical/logical identity mismatch, erased causes, false currentness, positional work and filtered-
inventory gaps. Corrected owners/#271/#239; no Rust/run.

### R011 — preparation/publication convergence

At `7c7fd36c`, inspected materializer/daemon preparation, publication/control/registry and redb. Found profile
test/limit coupling, budget gap, ABA guard conflation, success-only publication and non-durable build selection.
Corrected owners/#270/#331/#261; no Rust/run.

### R012 — window admission/evidence domains

At `e5350809`, inspected continuation/prepared output/session/protocol, DIRECT model/spine and validator. Found
post-allocation admission, count-only retention, lazy expiry, port errors flattened to gaps and raw-only ranges.
Preserved useful output-before-cursor commit and truthful completeness checks. Corrected owners/#275/#293; no
Rust/run.

### R013 — handles/client transport

At `44d7416c`, inspected result handles/expansion/source fence, runtime/session/output and full CLI client. Found
catalogue-wide scans, latest-source recheck, arbitrary raw expansion, per-page TTL renewal, partial multi-line
visibility, no active Cancel path and incomplete child cleanup. Corrected owners/#275/#279/#280/#293/#332; no
Rust/run.

### R014 — legacy GC/storage status/restore authority

At `2b37f5ef`, inspected maintenance/guard/status query callers, generic retention and feature-gated restore.
Found append-log GC retaining all historical revisions while missing current roots, repeated query-time storage
walks and same-installation restore authority. Corrected owners/#243/#244/#279/#302; no Rust/run.

### R015 — residual structural and lifecycle proof consumers

At `8cf331a4`, inspected current manifests/test tree, remaining ownership tests and lifecycle fixtures. Removed ten
pure source-text tests plus five obsolete restore/retention/purge proof suites (123,096 source bytes). Preserved
causal filesystem/GC/archive requirements in `TEST_CUTOVER.md`; no production/Cargo/run claim.

### R016 — isolated snapshot/sealed applications (2026-10-23)

Inspected exact head `bf2bde299b3f009527397571bd9baeea51fde840`:

- daemon and CLI manifests;
- daemon `src/main.rs`, `control_store.rs`, `snapshot.rs`, `snapshot/**`, `lexical/**` and all six `src/bin/eliot-search-sealed-*.rs` entrypoints;
- CLI `src/main.rs`;
- `tests/legacy_snapshot_isolation.rs`;
- caller searches for `DevelopmentControlStore`, `LexicalIndex`, `SnapshotIndex`, snapshot target names and sealed entrypoint filenames;
- current daemon owner map and `TEST_CUTOVER.md`.

Confirmed:

1. `eliot-search-snapshotd` is an independent loopback TCP/token application with a private alternating control
   store, snapshot manifest/revision tree and complete in-memory BM25 index. Its status explicitly reports
   `production_ready:false`; current `entry.rs` does not import this snapshot/lexical stack.
2. CLI `eliot-search-snapshot` only speaks that obsolete endpoint/token protocol and has no caller after the
   daemon target is removed.
3. The six sealed files are independent Windows test applications for owner/catalog/store/transaction/access
   chains. Exact filename searches found their Cargo declarations and historical docs, not current product calls.
4. `legacy_snapshot_isolation.rs` executes no behavior; it enforces the target declarations, module split, byte
   ceilings and local-index implementation.
5. The real causal requirements—stable bounded read, no-clobber object creation, link/reparse refusal, exact
   coordinates and truthful incomplete results—already belong to current source/object/query owners. The FNV
   snapshot identity, local BM25 and manual TCP/token protocol do not.

R016 removes the eight explicit test-target declarations, their exclusive source trees and the structural test.
It updates the daemon/test maps and registry. No workspace member, dependency or lockfile entry changes. The
primary `entry.rs`, shared sealed/migration modules and wave ladder remain for the next cut. No compiler, Clippy,
Windows or runtime result is claimed.
