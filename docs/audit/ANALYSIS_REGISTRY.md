# Analysis registry

Continuation memory for the refactor audit: exact inspected revisions/paths, findings, changes, execution
evidence and remaining uncertainty. It is not a permission gate, package-status database or substitute for owner
instructions.

**Original public baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Latest inspected source:** `2b37f5ef1e8cc755bbcd8c0e1f7efc840eff8cf3` (2026-10-23, R014)  
**Historical qualification archive:** `2f9c9720f3e14799a0f8dfefa00077d2fa6a9887/qualification`  
**Rule:** resume from the actual PR head and inspect intervening commits. Bind every compiler/runtime claim to
exact SHA, command and target. Row counts are not a completion percentage.

## Status vocabulary

- **CHECKED** — named static surface inspected for the stated question; not every line/runtime property.
- **PARTIAL** — useful findings exist but named code/callers/reconciliation remain.
- **PENDING** — insufficient inspection to direct a safe change.
- **RUNTIME-ONLY** — implementation/execution required; prose cannot establish it.
- **PROTECTED** — unpublished work is not inspected/overwritten; consume accepted public handoff.

## Coverage ledger

| ID | Area | Inspected surface | Status | Current conclusion | Remaining |
|---|---|---|---|---|---|
| A001 | Documentation authority/consistency | Root authority plus current source/preparation/Qdrant/publication/query/protocol/CLI/maintenance owners | PARTIAL | Historical parallel specs removed. R009-R014 found contradictions inside replacement docs and moved algorithms back to one owner. R014 reconciles maintenance/restore with actual legacy code. | Continue owner/issue sweep only when concrete source is inspected. |
| A002 | Historical governance data | swarm, generated maps, packets/handoffs, context/ticket artifacts | CHECKED | Removed from current tree; Git history is archive. | Code remnants tracked below. |
| A003 | Workflows/wrappers | workflows, PowerShell wrappers, temporary verifier | CHECKED | Old gates/wrappers removed; Actions manual-only. | Recheck when workflows change. |
| A004 | Workspace inventory | Root manifest and 51 current package manifests | CHECKED | Every member has receiving/removal/optional/conditional disposition; no target count. | Physical product-package cutovers. |
| A005 | Entrypoints/product | Daemon/CLI interceptors, one-shot scan, source sync, prep CLI, DIRECT runtime/client/maintenance | PARTIAL | One-shot scan is a narrow runnable island. Source sync/target product remain disconnected. Query output currently invokes full storage inventory. | One connected default daemon/CLI; remove interceptors and hot-path diagnostics. |
| A006 | Root/config/control | Root owner/config; old journal/snapshot/registry restore; redb snapshot; R014 stopped-root/archive boundary | PARTIAL | One root guard and typed current state selected. Archive/import must not copy locks/external authority. | Implement/execute typed state, Windows durability and archive atomicity. |
| A007 | Source acquisition/registry/reconcile | Admission stack; comparator/currentness/catalog/sync; ignore donor | PARTIAL | R010 found wrong-root absence, dropped compound update, identity mismatch, erased reasons and false currentness. Docs fixed; Rust not. | Real source/sync cutover and accepted #598 handoff. |
| A008 | Private storage/key overlap | Public #598 contract only | PROTECTED | Obsolete six-domain/purge/compatibility programme not inherited. | Consume accepted handoff; do not inspect/reset/duplicate private work. |
| A009 | Preparation/identity | Materializer profile/request/pipeline/read/model; unitizer/point identity | PARTIAL | Test/limit profile coupling, budget gap, empty refusal and ABA/stale installation distinctions recorded. | Implement current preparation and focused cases. |
| A010 | Qdrant/donors | Bridge and Qdrant 1.19 mutation/readback/BM25/tokenizer/scoped-IDF | CHECKED for named static questions | Gate removed; exact-name/scoped-IDF/late-write limits explicit. Real connection unimplemented. | Connect and run relevance/scale/late-write cases. |
| A011 | Publication/view/cleanup | Competing machines; redb view; late writers; cleanup; R014 restore boundary | PARTIAL | Two-sided reconciliation, immutable retrieval, fixed pending input and fresh-generation recovery selected. Copied pending/cleanup has no destination authority. | Implement crash/view/cleanup/restore behavior. |
| A012 | Query/evidence/window | Query stack, validator/projector, exact helper, continuation/result handles | PARTIAL | R012/R013 found late admission, failure-domain collapse, coordinate mismatch, source-fence scans, latest-source checks, arbitrary expansion and split lifetimes. | Implement one pinned QueryWindow and real evidence reader. |
| A013 | Protocol/client/CLI | Provider/line stacks, DIRECT session/output/client | PARTIAL | No active Cancel reader; multi-line partial visibility; per-line not aggregate bounds; post-spawn cleanup holes. | Framed terminal, serviced control path and owned-child guard; execute platform cases. |
| A014 | Product depth | Orientation/comparison/document requirements and old optional scaffolds | PARTIAL | User behaviors retained; deleted scaffolds are not delivered capabilities. | Select concrete producers/consumers and assess quality. |
| A015 | Tests/governance code | xtask/ledger/workspace/lock; source-shape tests; R014 lifecycle fixtures/features | PARTIAL | xtask and inspected structural tests removed. R014 identifies `wave7-lifecycle`, restore/purge/retention process and ownership fixtures as exclusive old-platform consumers. | Remove those with code cut; search residual structural assertions. |
| A016 | Donors | redb, Qdrant, Aho-Corasick, Git/ripgrep/ignore, Vec | PARTIAL | Concrete seams only; no whole-donor claims. | Revisit only for selected implementation decisions. |
| A017 | Qualification tree | Complete tree and representative reports/consumers | CHECKED | Archived; only policy/blocker summary remains. | Runtime ceremony under A022. |
| A018 | Auxiliary docs | Package/binary root filename sweep | CHECKED for filename/disposition | Duplicate wave/profile/DIRECT specs removed; distinct diagnostics retained. | Semantic consistency remains A001. |
| A019 | Issues/PR handoff | #352/#214/#525 and affected product issues through R014 | PARTIAL | Named issues link current branch owners rather than stale pinned copies. | Continue only for changed owners; no whole-queue claim. |
| A020 | Runtime evidence | Windows, blocked I/O, Qdrant late writes/IDF, relevance/scale, reader growth, restart/restore, child/GC/archive | RUNTIME-ONLY | Static counterexamples define execution work, not readiness. | Implement/run exact SHA/commands/raw output. |
| A021 | Compiler baseline | Recorded Ubuntu Rust 1.98 runs through 39872058276 | PARTIAL | Latest verified run covers `search-qdrant-bridge --lib` at `7d92fb8b`; earlier daemon check failed. R009-R014 docs-only. | Actual consumers, tests, daemon and Windows. |
| A022 | Runtime qualification/profile identity | Qdrant gate; lexical fixtures/identity; materializer analogue | PARTIAL | Qdrant activation token removed; lexical fixture-backed activation remains. | Remove through real callers, retain causal tests. |
| A023 | Local GC/storage diagnostics | Legacy maintenance, guard, storage status, query refresh callers | PARTIAL | Legacy GC replays all history and keeps every logged revision; misses new root owners. Query/page output runs repeated full storage inventories. Target is one typed stopped-root mark/sweep and explicit deep verify. | Implement concrete current-format collector and remove hot-path scans. |
| A024 | Backup/restore/import | Retention recipe; feature-gated restore composition/lifecycle; issues #243/#244 | PARTIAL | Old restore requires same installation and copied generation/purge/key authority. Target archive marker is written last; import creates fresh destination/generation and can rebuild one validated archived published or desired-current baseline. | Implement/corrupt-copy/interrupted/protected restore cases; delete old platform. |

## Pass log

### R001-R005 — remove parallel authority and duplicate specifications

Removed Architecture 8.4 master, waves, swarm/status/coverage/receipt data, generated maps, wrappers and package-
local W7/W8/W10/P18/journal/profile copies. Corrected local GC from online gate machinery to stopped-root
maintenance. History remains in Git.

### R006 — xtask/digest governance

Removed xtask, 3.57-MB ledger, workspace member and `cargo_metadata`; regenerated lockfile. Run 39868374272
passed recorded metadata/search-contracts/strict Clippy under Rust 1.98. Broad daemon check failed.

### R007-R008 — source-shape tests and stale qualification

Removed inspected topology tests: 11 Qdrant, 5 evaluation, 33 daemon and 17 preparation/store. Kept behavior
suites. Run 39869930525 passed named-library check/strict Clippy. R008 corrected stale references and located
remaining runtime qualification.

### R009 — Qdrant/lexical qualification

At `7d92fb8b`, confirmed Qdrant gate removal; found lexical fixture/resource data in profile identity and runtime
admission. Run 39872058276 passed check/Clippy only for `search-qdrant-bridge --lib`. No daemon/live Qdrant.

### R010 — source observation/currentness

At `c0fcf294`, inspected reconcile/currentness/catalog/sync and ignore source. Found wrong-root absence, compound
update loss, physical/logical identity mismatch, erased causes, false currentness, positional work and filtered-
inventory gaps. Corrected owner docs/#271/#239; no Rust/run.

### R011 — preparation/publication convergence

At `7c7fd36c`, inspected materializer and daemon preparation, publication/control/registry and redb source. Found
profile test/limit coupling, budget gap, ABA guard conflation, success-only publication and non-durable build
selection. Corrected owners/#270/#331/#261; no Rust/run.

### R012 — window admission/evidence domains

At `e5350809`, inspected continuation/prepared output/session/protocol, DIRECT model/spine and validator. Found
post-allocation admission, count-only retention, lazy expiry, port errors flattened to gaps and raw-only ranges.
Preserved useful output-before-cursor commit and completeness checks. Corrected owners/#275/#293; no Rust/run.

### R013 — handles/client transport

At `44d7416c`, inspected result handles/expansion/source fence, runtime/session/output and full CLI client. Found
catalogue-wide scans/global invalidation, latest-source recheck, arbitrary raw expansion, per-page TTL renewal,
partial multi-line visibility, no active Cancel path and incomplete child cleanup. Corrected owner docs and
#275/#279/#280/#293/#332 in `2b37f5ef`; no Rust/run.

### R014 — legacy GC, storage-status hot path and restore authority (2026-10-23)

Inspected exact head `2b37f5ef1e8cc755bbcd8c0e1f7efc840eff8cf3`:

- `crates/search-runtime/search-retention/FUNCTIONS.md`, README/AGENTS and source tree;
- `search-retention/src/lib.rs` lines 1-260 plus source sizes for `purge.rs`/`sweep.rs`;
- daemon `maintenance.rs` in full, `maintenance_guard.rs`, `storage_security.rs` and
  `public_runtime_service/kernel/{diagnostics,mutation}.rs` in full;
- daemon restore facade/tree and `restore_composition/kernel/lifecycle.rs` in full;
- daemon entry/manifest feature boundary and caller searches for restore/retention;
- issues #243/#244/#302 and current daemon/query owners.

Confirmed:

1. `verify_complete_log` inserts the revision from every append-only event into the live set regardless of
   current/retired state. The legacy GC therefore cannot reclaim superseded logged revisions, while it knows
   nothing about preparation/published/pending/cleanup references.
2. `guarded_collect_orphan_revisions` repeats preview/apply full scans. A following storage refresh scans again.
   Safety checks are useful; the repeated historical reference algorithm is not.
3. `StorageSecurityStatus::inspect` calls that GC inventory, scans revision objects again for protected headers
   and scans preparation trees. Query/page/continue/expand call `refresh_storage` before output, so interactive
   work can be proportional to all history/objects merely to emit storage metadata.
4. Header/suffix inventory does not prove all ciphertext is decryptable/authentic. It must not be a per-response
   `encrypted_at_rest` credential. Real selected reads verify actual objects; deep inventory is explicit.
5. `search-retention` remains about 82 KiB of generic lifecycle source. Feature-gated daemon restore composition
   adds a receipt/cutover/key-migration state machine. Caller search found restore use in feature/test fixtures,
   not a normal product command.
6. Old `stage_restore` requires destination root/incarnation/epoch and physical root to match the export and
   carries copied collection/visible epoch/purge/key authority. This directly conflicts with fresh-destination
   import. Its sole-copy deletion and generic key migration are not donor behavior.
7. The replacement needed a concrete archive commit boundary: stage outside the root, verify copied state,
   sync where real, then publish an archive-complete marker last. Restore may rebuild a fully validated archived
   published baseline or desired-current baseline, never a mixture, then reconcile newer state.

Updated retention/daemon owner docs and registry. Issue bodies are reconciled separately. Static source/design
findings only: no Rust/Cargo/workflow/private #598 changes, build, Clippy, filesystem copy, GC or restore run.
Search results are not exhaustive proof of every feature caller. A023/A024 remain PARTIAL; platform/runtime work
is A020.
