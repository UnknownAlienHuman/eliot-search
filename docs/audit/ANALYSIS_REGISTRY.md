# Analysis registry

Continuation memory for the refactor audit: exact inspected revisions/paths, findings, changes, execution
evidence and remaining uncertainty. It is not a permission gate, package-status database or substitute for owner
instructions.

**Original public baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Latest inspected source:** `151456672ea4ec8773ed65c5017cb63989912d30` (`R018`, 2026-10-23)  
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
| A001 | Documentation authority/consistency | Root authority; architecture/implementation; current source, preparation, Qdrant, publication, query, protocol, CLI, maintenance and test owners | PARTIAL | Historical parallel specifications were removed. R009-R018 repeatedly found contradictions in replacement documents and moved algorithms/causal cases back to current owners. | Continue the surviving owner/issue sweep only with concrete source inspection; do not create another global specification. |
| A002 | Historical governance data | Swarm, generated maps, packets/handoffs, context/ticket artifacts | CHECKED | Removed from the current tree; Git history is the archive. | Code remnants are tracked below. |
| A003 | Workflows/wrappers | Workflows, PowerShell wrappers and temporary qualification verifier | CHECKED | Old gates/wrappers removed; Actions remain manual-only. | Recheck only when workflows change. |
| A004 | Workspace/package inventory | Root manifest and 51 current workspace members | CHECKED for inventory | Every member has a receiving/removal/optional/conditional disposition. R017 intentionally leaves an empty `search-retention` member until the manifest/lock cut. Package count itself is not a target. | Physical cutovers and Cargo pruning. |
| A005 | Entrypoints/runnable product | Daemon/CLI interceptor chains, one-shot scan, source sync, preparation CLI, DIRECT runtime/client/maintenance; snapshot/sealed targets; feature-gated restore; R018 process proofs | PARTIAL | One-shot scan is a narrow runnable island. Separate snapshot/sealed/restore applications and disconnected acceptance fixtures are removed. The primary daemon still has an interceptor chain and disconnected source→Qdrant→query path. | One default daemon/CLI application; remove interceptors, compatibility commands, shared dead modules and hot-path diagnostics. |
| A006 | Root/config/control | Root owner/config; old journal/snapshot/registry restore; redb 2.6.3 snapshot; stopped-root/archive boundary | PARTIAL | One root guard and typed current state selected. A process-held read snapshot is not durable build selection; copied locks/external authority are not restore state. | Implement/execute typed state, full-build input capture, Windows lock/durability and archive publication. |
| A007 | Source acquisition/registry/reconciliation | Admission stack; comparator; currentness/catalog/sync; ignore donor | PARTIAL | R010 found wrong-root absence, dropped compound update, physical/logical identity mismatch, erased reasons, positional work and false currentness. Instructions were corrected; Rust remains unfixed. | Real source/object/current-row cutover, accepted #598 handoff and focused cases. |
| A008 | Private storage/key overlap | Public #598 contract only | PROTECTED | The obsolete six-domain/purge/compatibility programme is not inherited. | Consume the accepted handoff; do not inspect/reset/duplicate unpublished work. |
| A009 | Preparation/identity | Materializer profile/request/pipeline/read/model; unitizer/point identity | PARTIAL | Test/resource data contaminates profile identity; execution budget can be tighter than prevalidation; empty input is still refused; reuse, current installation guard and retry eligibility are different facts. | Implement current preparation and verify empty/gap/budget/ABA/stale completion. |
| A010 | Qdrant/donors | Bridge; Qdrant 1.19 mutation/readback/BM25/tokenizer/scoped-IDF; activation gate/connect; R018 live/oracle fixtures | CHECKED for named static questions | QualifiedGate was removed. R016 deleted local BM25 fallback. R018 deletes stale QualifiedGate live parity, the in-memory query/rebuild oracle and duplicate manual REST product half. Causal live-Qdrant cases remain assigned to the real bridge/publication path. | Connect the real product path and execute relevance/scale/late-write/rebuild cases. |
| A011 | Publication/view/cleanup | Competing publication machines; redb view; late writers; cleanup/restore boundary; R018 rebuild fixture | PARTIAL | Two-sided desired/published reconciliation, immutable retrieval state, durable captured build inputs and isolated-generation recovery were selected. The old rebuild/reclaimer proof fixture is removed; full readback, fresh generation, old-view guard and unknown-write cases remain requirements. | Implement crash/view/cleanup/rebuild/restore behavior and execute it. |
| A012 | Query/evidence/window | Query stack, validator/projector, exact helper, continuation/result handles; R018 indexed query/handle fixtures | PARTIAL | R012/R013 found post-allocation admission, collapsed failure domains, raw-only coordinates, whole-source fences, latest-source rechecks, arbitrary expansion and split lifetimes. R018 removes the in-memory proof chain and random-handle process suite; causal scope/evidence/window cases remain. | One pinned `QueryWindow`, direct Qdrant path and real evidence reader; remove production catalogue/proof families. |
| A013 | Protocol/client/CLI | Provider/line stacks, DIRECT session/output/client; TCP/token snapshot; R018 pairing/provider/indexed-unavailable fixtures | PARTIAL | No active Cancel reader; partial multi-line visibility; per-line not aggregate bounds; manual allocation and child cleanup gaps. R018 removes pairing/token/envelope/replay process systems and unavailable qualification proof. | One capped framed terminal, serviced control path, owned-child guard; remove production loopback/pairing stack and execute platform cases. |
| A014 | Product depth | Old orientation/comparison/document requirements; enrichers/resolvers/workers/overlay; recipe process fixture | PARTIAL | User-visible goals are retained; deleted scaffolds are not capabilities. R018 removes the fixed eleven-recipe acceptance fixture and preserves tasks as direct-handler requirements. | Select concrete producers/consumers and assess quality after baseline Search works. |
| A015 | Tests/governance code | xtask/ledger; source-shape/lifecycle/snapshot removals; R018 exact process-proof files | PARTIAL | R015-R017 removed structural/lifecycle/snapshot systems. R018 removes nine disconnected process proofs totaling 243,346 source bytes. Mixed migration tests still combine real I/O cases with source assertions; other current behavior tests remain. | Extract causal cases from mixed fixtures; remove remaining obsolete process proofs with production owners; run changed-owner compiler/Clippy. |
| A016 | Donors | redb, Qdrant, Aho-Corasick, Git/ripgrep/ignore and stable Vec behavior | PARTIAL | Donor behavior is read only at concrete seams. No whole-donor or upgrade claim. | Revisit only for a selected implementation question. |
| A017 | Qualification tree | Complete tree and representative reports/consumers | CHECKED | Archived in Git; current tree retains only policy/blocker summary. | Runtime ceremony is A022. |
| A018 | Auxiliary docs | Package/binary root filename sweep and standard/nonstandard supplements | CHECKED for filename/disposition | Duplicate wave/profile/DIRECT specifications removed; distinct diagnostics retained. | Semantic consistency remains A001. |
| A019 | Issues/PR handoff | #352/#214/#525, affected product issues and PR through R018 | PARTIAL | Named issues link current owners instead of stale pinned algorithms. | Update only when an owner or inspected source changes; no whole-queue claim. |
| A020 | Runtime evidence | Windows, blocked I/O, live Qdrant late writes/IDF/relevance/scale, reader growth, restart/restore, child/GC/archive | RUNTIME-ONLY | Deleted fixture claims are not runtime evidence. Static counterexamples and moved causal cases define required runs. | Implement and run with exact SHA/commands/raw outcome. |
| A021 | Compiler baseline | Ubuntu Rust 1.98 runs through `39872058276` | PARTIAL | Latest verified run covers `search-qdrant-bridge --lib` at `7d92fb8b`; an earlier broad daemon check failed. R009-R018 make no new compiler claim. R018 deletes integration-test source only. | Actual consumers, daemon, current tests and Windows. |
| A022 | Runtime qualification/profile identity | Qdrant gate; lexical fixture identity; materializer analogue; R018 stale qualification fixtures | PARTIAL | Qdrant activation token and its stale process fixtures are gone. Lexical fixture-backed activation and profile/test coupling remain. | Remove ceremony through real callers; retain causal examples as ordinary tests. |
| A023 | Local GC/storage diagnostics | Legacy maintenance/guard/status and query refresh callers; generic retention source | PARTIAL | Generic sweep/purge source is deleted; actual typed stopped-root collector and removal of query-time inventories are still unimplemented. | Implement typed mark/sweep and remove recursive inventory from query/output. |
| A024 | Backup/restore/import | Generic retention crate; feature-gated restore composition; #243/#244 | PARTIAL | Old same-installation implementation is deleted. Target remains completed archive plus fresh destination/Qdrant generation. | Implement partial-marker/corrupt-copy/interrupted/protected cases; remove empty Cargo shell. |
| A025 | Feature/harness topology | Root/daemon/CLI manifests; `wave1`–`wave7`; snapshot/sealed/restore targets; R018 process fixtures | PARTIAL | Standalone targets, lifecycle implementation and nine self-contained proof suites are removed. Feature/dependency shells and production old query/provider/rebuild/pairing/handle modules remain. | Dedicated Cargo/production cuts with minimal metadata/check/Clippy. |

## Remaining work queue

This is the current ordered backlog, not a gate:

1. **Cargo feature/package cut:** remove `wave7-lifecycle`, `full-baseline`, empty `search-retention` daemon/root
   entries and lock record; then collapse `wave1`–`wave6` only with their actual consumers.
2. **Entrypoint/provider cut:** remove interceptor chain, legacy loopback/pairing/token provider and shared sealed/
   migration compatibility modules after causal Windows/object cases move.
3. **Source/object/current state:** accepted #598 handoff, retained bytes before current-head commit, real sync,
   current-row recovery and preparation scheduler.
4. **Qdrant/publication:** connect one runtime; immutable published retrieval state; pending/readback/late-writer
   handling; atomic view/evidence commit, fresh-generation rebuild and finite cleanup.
5. **Query/session:** direct scoped BM25, pinned composite evidence lookup, real retained read, one `QueryWindow`,
   framed terminal, active Cancel and owned-child shutdown; delete production proof/catalogue stacks.
6. **Maintenance/archive/import:** typed stopped-root GC; archive-complete marker last; fresh destination identity and
   Qdrant generation; no copied external authority.
7. **Residual tests/qualification:** mixed migration source assertions, lexical fixture activation, remaining old
   command/quarantine fixtures and obsolete package consumers.
8. **Product depth/runtime evidence:** exact/navigation/comparison/documents/MCP through real producers, then
   Windows durability/cancellation, live Qdrant unknown writes/IDF/relevance/scale, restart, reader growth,
   GC/archive/import and connected default-binary smoke.

## Pass log

### R001-R005 — remove parallel authority and duplicate specifications

Removed Architecture 8.4 master, waves, swarm/status/coverage/receipt data, generated maps, wrappers and
package-local W7/W8/W10/P18/journal/profile copies. Exact history remains in Git.

### R006 — xtask/digest governance

Removed xtask, 3.57-MB ledger, workspace member and `cargo_metadata`; regenerated lockfile. Run `39868374272`
passed recorded metadata/`search-contracts`/strict Clippy. Broad daemon check failed.

### R007-R008 — source-shape tests and stale qualification

Removed 66 inspected topology tests while retaining behavior suites. Run `39869930525` passed named-library
check/strict Clippy. Corrected stale references and located remaining runtime qualification.

### R009 — Qdrant/lexical qualification

At `7d92fb8b`, confirmed Qdrant gate removal; found lexical fixture/resource data in profile identity and runtime
admission. Run `39872058276` passed check/Clippy only for `search-qdrant-bridge --lib`.

### R010 — source observation/currentness

At `c0fcf294`, found wrong-root absence, compound update loss, physical/logical identity mismatch, erased causes,
false currentness, positional work and filtered-inventory gaps. Corrected owners/issues; no Rust/run.

### R011 — preparation/publication convergence

At `7c7fd36c`, found profile test/limit coupling, budget gap, ABA guard conflation, success-only publication and
non-durable build selection. Corrected owners/issues; no Rust/run.

### R012 — window admission/evidence domains

At `e5350809`, found post-allocation admission, count-only retention, lazy expiry, port errors flattened to gaps
and raw-only coordinate assumptions. Corrected owners/issues; no Rust/run.

### R013 — handles/client transport

At `44d7416c`, found catalogue-wide scans, latest-source recheck, arbitrary expansion, per-page TTL renewal,
partial multi-line visibility, no active Cancel path and incomplete child cleanup. Corrected owners/issues.

### R014 — legacy GC/storage status/restore authority

At `2b37f5ef`, found append-log GC retaining all historical revisions while missing current roots, repeated
query-time storage walks and same-installation restore authority. Corrected owners/issues.

### R015 — residual structural/lifecycle consumers

At `8cf331a4`, removed ten source-text tests and five restore/retention/purge proof suites (123,096 source bytes).

### R016 — isolated snapshot/sealed applications

At `bf2bde29`, removed eight explicit snapshot/sealed test applications, private snapshot/control/local-BM25
source and structural isolation test (5,765 lines).

### R017 — restore and generic lifecycle implementation

At `53ab0ea3`, removed daemon restore source and 82-KiB generic retention/purge/sweep/restore code (3,218
production Rust lines), leaving a temporary empty manifest/lock shell. No run.

### R018 — disconnected query/provider/rebuild proof fixtures (2026-10-23)

Inspected exact head `151456672ea4ec8773ed65c5017cb63989912d30`:

- `indexed_query_process.rs` in full enough to identify its complete in-memory oracle/plan/access/executor/
  validator/projector chain and all terminal cases;
- `product_spine_process.rs` header, DIRECT fixture and live-Qdrant/restart/loss/rebuild/unknown-write cases;
- `rebuild_process.rs` header, retained-manifest/oracle/pin/reclaimer setup and restart/readback/pin/cleanup cases;
- `qdrant_grpc_parity_process.rs` in full;
- `indexed_provider_process.rs` in full;
- `pairing_process.rs` through its live loopback/pairing/vault fixture;
- `provider_process.rs` through its canonical envelope/pairing/token proxy fixture;
- `recipes_process.rs` through registry/support matrix and DIRECT process setup;
- `live_handles_process.rs` through served-session, token, page and expansion fixture setup;
- current query/window/protocol/CLI/publication/bridge/test-cutover owners.

Confirmed:

1. `indexed_query_process` explicitly does not drive the served daemon; it proves the old proof stack against an
   in-memory oracle and caller-supplied readback double.
2. `product_spine_process` explicitly joins two halves and carries a duplicate HTTP/Qdrant client; it admits no
   one-command indexed product path.
3. `rebuild_process` uses the same oracle and old plan/pin/reclaimer receipt composition, not current publication
   state or live restart recovery.
4. `qdrant_grpc_parity_process` still constructs `QualifiedGate`, which the selected bridge has removed. The real
   bridge owns live Qdrant tests.
5. `indexed_provider_process` only proves the qualification-gated operation is unavailable.
6. `pairing_process` and `provider_process` prove token files, vault leases, pairing MACs, replay envelopes and
   loopback TCP that trusted owned-child stdio explicitly rejects.
7. `recipes_process` freezes eleven recipe IDs/support matrices and maps them onto old DIRECT commands. Product
   tasks survive as direct operations, not a recipe platform.
8. `live_handles_process` exclusively protects random source handles, full-source fences and split continuation/
   handle lifetimes that R012/R013 selected for replacement.
9. The useful causal cases can be stated without these architectures: real scope/IDF/readback; unknown writes;
   lost-index rebuild; old-view cleanup; missing evidence; non-exhaustive top-k; fixed window lifetime; partial
   terminal commit; active cancellation and owned-child cleanup.

R018 deletes the nine files (243,346 source bytes), updates test/registry maps and does not change production
Rust, Cargo, dependencies, lockfile or private #598 work. No compiler/Clippy/Qdrant/process result is claimed.
