# Analysis registry

Continuation memory for the refactor audit: exact inspected revisions/paths, findings, changes, execution
evidence and remaining uncertainty. It is not a permission gate, package-status database or substitute for owner
instructions.

**Original public baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Latest inspected source:** `8cf331a44decded2c84c7f579dcbf8434e040dc7` (`R015`, 2026-10-23)  
**Historical qualification archive:** `2f9c9720f3e14799a0f8dfefa00077d2fa6a9887/qualification`  
**Rule:** resume from the actual PR head, inspect intervening commits, and bind every compiler/runtime claim to
an exact SHA, command and target. Row counts are not a completion percentage.

## Status vocabulary

- **CHECKED** — named static surface inspected for the stated question; not every line or runtime property.
- **PARTIAL** — useful findings exist, but named code/callers/reconciliation remain.
- **PENDING** — insufficient inspection to direct a safe change.
- **RUNTIME-ONLY** — implementation/execution required; prose cannot establish it.
- **PROTECTED** — unpublished work is not inspected or overwritten; consume its accepted public handoff.

## Coverage ledger

| ID | Area | Inspected surface | Status | Current conclusion | Remaining |
|---|---|---|---|---|---|
| A001 | Documentation authority/consistency | Root authority; architecture/implementation; current source, preparation, Qdrant, publication, query, protocol, CLI and maintenance owners | PARTIAL | Historical parallel specifications were removed. R009-R014 repeatedly found contradictions in the replacement documents and moved each algorithm back to one owner. | Continue the surviving owner/issue sweep only with concrete source inspection; do not create another global specification. |
| A002 | Historical governance data | Swarm, generated maps, packets/handoffs, context/ticket artifacts | CHECKED | Removed from the current tree; Git history is the archive. | Code remnants are tracked below. |
| A003 | Workflows/wrappers | Workflows, PowerShell wrappers and temporary qualification verifier | CHECKED | Old gates/wrappers removed; Actions remain manual-only. | Recheck only when workflows change. |
| A004 | Workspace/package inventory | Root manifest and 51 current workspace members | CHECKED for inventory | Every member has a receiving/removal/optional/conditional disposition. Package count itself is not a target. | Physical cutovers and Cargo pruning. |
| A005 | Entrypoints/runnable product | Daemon/CLI interceptor chains, one-shot scan, source sync, preparation CLI, DIRECT runtime/client/maintenance | PARTIAL | One-shot scan is a narrow runnable island. Source sync and the target source→Qdrant→query path remain disconnected. Current query uses legacy line output, handles and deep storage inventory. | One default daemon/CLI application; remove interceptors, compatibility commands and hot-path diagnostics. |
| A006 | Root/config/control | Root owner/config; old journal/snapshot/registry restore; redb 2.6.3 snapshot; stopped-root/archive boundary | PARTIAL | One root guard and typed current state selected. A process-held read snapshot is not durable build selection; copied locks/external authority are not restore state. | Implement/execute typed state, full-build input capture, Windows lock/durability and archive publication. |
| A007 | Source acquisition/registry/reconciliation | Admission stack; comparator; currentness/catalog/sync; ignore donor | PARTIAL | R010 found wrong-root absence, dropped compound update, physical/logical identity mismatch, erased reasons, positional work and false currentness. Instructions were corrected; Rust remains unfixed. | Real source/object/current-row cutover, accepted #598 handoff and focused cases. |
| A008 | Private storage/key overlap | Public #598 contract only | PROTECTED | The obsolete six-domain/purge/compatibility programme is not inherited. | Consume the accepted handoff; do not inspect/reset/duplicate unpublished work. |
| A009 | Preparation/identity | Materializer profile/request/pipeline/read/model; unitizer/point identity | PARTIAL | Test/resource data contaminates profile identity; execution budget can be tighter than prevalidation; empty input is still refused; reuse, current installation guard and retry eligibility are different facts. | Implement current preparation and verify empty/gap/budget/ABA/stale completion. |
| A010 | Qdrant/donors | Bridge; Qdrant 1.19 mutation/readback/BM25/tokenizer/scoped-IDF; activation gate/connect | CHECKED for named static questions | QualifiedGate was removed. Exact-name/tokenizer, scoped-IDF cost and late-writer limitations are explicit. | Connect the real product path and execute relevance/scale/late-write cases. |
| A011 | Publication/view/cleanup | Competing publication machines; redb view; late writers; cleanup and restore boundary | PARTIAL | Two-sided desired/published reconciliation, immutable retrieval state, durable captured build inputs and isolated-generation recovery were selected. Readback is not writer terminality. | Implement crash/view/cleanup/rebuild/restore behavior and execute it. |
| A012 | Query/evidence/window | Query stack, validator/projector, exact helper, continuation and result-handle catalogues | PARTIAL | R012/R013 found post-allocation admission, collapsed failure domains, raw-only coordinates, whole-source fences, latest-source rechecks, arbitrary expansion and split lifetimes. | One pinned `QueryWindow`, direct Qdrant path and real evidence reader; remove both catalogue families. |
| A013 | Protocol/client/CLI | Provider/line stacks, DIRECT session/output/client | PARTIAL | No active Cancel reader; multi-line partial results become visible before terminal; bounds are per line; manual strings allocate first; child cleanup has escape paths. | One capped framed terminal, serviced control path, owned-child guard and platform execution. |
| A014 | Product depth | Old orientation/comparison/document requirements; enrichers/resolvers/workers/overlay | PARTIAL | User-visible goals are retained; deleted scaffolds are not capabilities. | Select concrete producers/consumers and assess quality after baseline Search works. |
| A015 | Tests/governance code | xtask/ledger; previous source-shape removals; R015 exact current daemon/lifecycle tests | PARTIAL | xtask/ledger and 66 prior structural tests were removed. R015 removes ten more source-text topology tests and five obsolete lifecycle proof tests, 123,096 source bytes total. Mixed migration tests still combine real I/O cases with source assertions. | Extract causal cases, remove remaining source-text/process/harness proofs with their owners, then run changed-owner compiler/Clippy. |
| A016 | Donors | redb, Qdrant, Aho-Corasick, Git/ripgrep/ignore and stable Vec behavior | PARTIAL | Donor behavior is read only at concrete seams. No whole-donor or upgrade claim. | Revisit only for a selected implementation question. |
| A017 | Qualification tree | Complete tree and representative reports/consumers | CHECKED | Archived in Git; current tree retains only policy/blocker summary. | Runtime ceremony is A022. |
| A018 | Auxiliary docs | Package/binary root filename sweep and standard/nonstandard supplements | CHECKED for filename/disposition | Duplicate wave/profile/DIRECT specifications removed; distinct diagnostics retained. | Semantic consistency remains A001. |
| A019 | Issues/PR handoff | #352/#214/#525 and affected product issues through R015 | PARTIAL | Named issues link current branch owners instead of stale pinned algorithms. | Update only when an owner or inspected source changes; no whole-queue claim. |
| A020 | Runtime evidence | Windows, blocked I/O, Qdrant late writes/IDF, relevance/scale, reader growth, restart/restore, child/GC/archive | RUNTIME-ONLY | Static counterexamples define required runs; they do not establish readiness or reproduce every failure. | Implement and run with exact SHA/commands/raw outcome. |
| A021 | Compiler baseline | Ubuntu Rust 1.98 runs through `39872058276` | PARTIAL | Latest verified run covers `search-qdrant-bridge --lib` at `7d92fb8b`; an earlier broad daemon check failed. R009-R015 make no new compiler claim. | Actual consumers, daemon, tests and Windows. |
| A022 | Runtime qualification/profile identity | Qdrant gate; lexical fixture identity; materializer analogue | PARTIAL | Qdrant activation token removed; lexical fixture-backed activation and profile/test coupling remain. | Remove ceremony through real callers; retain causal examples as ordinary tests. |
| A023 | Local GC/storage diagnostics | Legacy maintenance/guard/status and query refresh callers | PARTIAL | Legacy GC keeps every logged revision yet misses current preparation/published/pending/cleanup roots. Query-time status repeats full log/object/preparation walks. | Implement typed stopped-root mark/sweep and remove recursive inventory from query/output. |
| A024 | Backup/restore/import | Generic retention crate; feature-gated restore composition; #243/#244 | PARTIAL | Old restore is same-installation receipt/cutover/key authority. Target is completed archive plus fresh destination/Qdrant generation, with copied pending/cleanup non-executable. | Implement partial-marker, corrupt-copy, interrupted import and protected-storage cases; remove old platform. |
| A025 | Feature/harness topology | Root/daemon/CLI manifests; `wave1`–`wave7`; eight explicit snapshot/sealed test targets; lifecycle fixtures | PARTIAL | Feature ladders compile disconnected applications. `product_targets.rs` explicitly forced obsolete harnesses to remain and was removed in R015. | Remove wave ladder, harness targets and exclusive source when current owners are connected; do not replace them with another acceptance profile. |

## Remaining work queue

This is the current ordered backlog, not a gate:

1. **Entrypoint and feature cut:** one daemon/CLI path; remove interceptor chain, wave ladder, eight harness-only
   snapshot/sealed targets and exclusive compatibility source.
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

### R015 — residual structural and lifecycle proof consumers (2026-10-23)

Inspected exact head `8cf331a44decded2c84c7f579dcbf8434e040dc7`:

- root, daemon, CLI and `search-retention` manifests;
- daemon test directory and exact source for `product_targets`, control-migration ownership, direct-preparation,
  direct-store registry, Git source, revision inventory/key/envelope, source composition and native identity tests;
- feature-gated daemon `restore_process`, `retention_process`, `purge_process`;
- package `purge_barriers`/`sweep` tests;
- mixed `control_migration_bootstrap_boundary` and `control_migration_inventory_owner_boundary`;
- current `TEST_CUTOVER.md` and issue #214.

Confirmed:

1. Ten remaining tests only read source/manifests and assert topology. `product_targets.rs` requires seven daemon
   and one CLI legacy harness targets to remain, directly opposing the implementation guide.
2. Five lifecycle tests exclusively exercise `wave7-lifecycle`, generic `search-retention`, receipt phases,
   same-installation restore and synthetic CAS/purge planes. They do not drive current-format GC/archive/import.
3. The two mixed Windows migration tests contain useful no-clobber, bounded-read, drift, alias and incomplete-
   metadata cases, but also source-text assertions. Their causal cases must move to current owners before their
   legacy harness is deleted.
4. Current Cargo still retains `search-retention`, `wave7-lifecycle`, `full-baseline` and eight explicit
   snapshot/sealed test targets. This is the next code/Cargo cut, not proof that the old topology is required.

R015 deletes the ten pure structural tests and five obsolete lifecycle proof tests (123,096 source bytes),
rewrites the test-cutover map and updates #214. It changes no production Rust/Cargo and performs no compiler,
Clippy or runtime run. A015/A025 remain PARTIAL because mixed fixtures, feature targets and old implementations
still exist.
