# Analysis registry

Continuation memory for the refactor audit: exact inspected revisions/paths, findings, changes, execution
evidence and remaining uncertainty. It is not a permission gate, package-status database or substitute for owner
instructions.

**Original public baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Latest inspected source:** `156528ad0a2ab64599d9cef01d3eeea2b6f11040` (`R020`, 2026-10-23)  
**Historical qualification archive:** `2f9c9720f3e14799a0f8dfefa00077d2fa6a9887/qualification`  
**Rule:** resume from the actual PR head, inspect intervening commits, and bind every compiler/runtime claim to
an exact SHA, command and target. Commit findings with their changes. Row counts are not a completion percentage.
Do not erase inspected-path evidence when condensing this register; retain an immutable link to the detailed pass.

## Status vocabulary

- **CHECKED** — named static surface inspected for the stated question; not every line or runtime property.
- **PARTIAL** — useful findings exist, but named code/callers/reconciliation remain.
- **PENDING** — insufficient inspection to direct a safe change.
- **RUNTIME-ONLY** — implementation/execution required; prose cannot establish it.
- **PROTECTED** — unpublished work is not inspected/overwritten; consume its accepted public handoff.

The table tracks areas, not individual defect resolution. A documented defect can be statically established
while its code correction remains unfinished. The separate queues below distinguish those cases from unread
source or an unresolved design question.

## Coverage ledger

| ID | Area | Inspected surface | Status | Current conclusion | Remaining |
|---|---|---|---|---|---|
| A001 | Documentation authority/consistency | Root authority; architecture/implementation; current source, preparation, Qdrant, publication, query, protocol, CLI, maintenance and test owners; R020 root README/QUICKSTART | PARTIAL | Historical parallel specifications were removed. R020 corrects claims about deleted Cargo surfaces and an unverified runnable checkout; local maintenance notes no longer describe removed code as current. | Surviving owner/issue sweep with concrete source inspection; no new global specification. |
| A002 | Historical governance data | Swarm, generated maps, packets/handoffs, context/ticket artifacts | CHECKED | Removed from the current tree; Git history is the archive. | Code remnants below. |
| A003 | Workflows/wrappers | Workflows, PowerShell wrappers and temporary verifier; R020 manual diagnostic | CHECKED | Old gates/wrappers removed; Actions manual-only. No push trigger added to compensate for missing dispatch access. | Recheck only when workflows change. |
| A004 | Workspace/package inventory | Root/package inventory; R020 root, daemon and empty retention manifests plus lockfile | CHECKED for named inventory | R020 removes the empty retention package, leaving 50 declared workspace members. Its seven dependencies are other retained members, not packages to delete transitively. Package count is not a target. | Remaining package cuts and actual Cargo resolution. |
| A005 | Entrypoints/runnable product | Daemon/CLI interceptors; one-shot scan; source sync; preparation; DIRECT service; snapshot/sealed/restore/loopback applications | PARTIAL | Snapshot/sealed/restore standalone paths and daemon TCP/token loopback are removed. The primary daemon still has overlapping interceptors and no connected source→Qdrant→query path. Latest cleanup did not establish that the narrow scan route currently builds/runs. | One default daemon/CLI application; remove remaining compatibility and connect current owners. |
| A006 | Root/config/control | Root owner/config; old journal/snapshot/registry restore; redb snapshot; stopped-root/archive boundary | PARTIAL | One root guard and typed current state selected. Process-held snapshots are not durable build selection; copied external authority is not restore state. | Implement typed state, captured build inputs, Windows lock/durability and archive publication. |
| A007 | Source acquisition/registry/reconciliation | Admission stack; comparator/currentness/catalog/sync; ignore donor | PARTIAL | R010 found wrong-root absence, compound update loss, physical/logical identity mismatch, erased causes, positional work and false currentness. Docs fixed; Rust remains. | Real source/object/current-row cutover, actual #598 handoff and focused cases. |
| A008 | Private storage/key overlap | Public #598 contract only | PROTECTED | Obsolete six-domain/purge/compatibility programme not inherited. | Consume accepted handoff; do not inspect/reset unpublished work or block unrelated work. |
| A009 | Preparation/identity | Materializer profile/request/pipeline/read/model; unitizer/point identity | PARTIAL | Test/resource data contaminates profile identity; budget preflight differs from execution; empty input refused; reuse/install/retry are distinct. | Implement current preparation and empty/gap/budget/ABA/stale-completion cases. |
| A010 | Qdrant/donors | Bridge; Qdrant 1.19 mutation/readback/BM25/tokenizer/scoped-IDF; old live/oracle fixtures | CHECKED for named static questions | QualifiedGate, local BM25 prototype, stale gate fixture, in-memory oracle and duplicate REST product half are removed. Root Cargo no longer demands an activation receipt for its SDK pin. | Connect real path; execute relevance/scale/late-write/rebuild. |
| A011 | Publication/view/cleanup | Publication machines; redb view; late writers; cleanup/restore; old rebuild fixture | PARTIAL | Two-sided reconciliation, immutable retrieval, captured build inputs and isolated-generation recovery selected. Old rebuild proof removed. | Implement crash/view/cleanup/rebuild/restore and execute. |
| A012 | Query/evidence/window | Query stack; validator/projector/exact; continuation/result handles; old indexed/handle fixtures | PARTIAL | Post-allocation admission, collapsed failures, raw-only coordinates, whole-source fences, latest-source rechecks, arbitrary expansion and split lifetimes identified. Old proof/handle fixtures removed. | One pinned QueryWindow, direct Qdrant/evidence reader; delete production proof/catalogue families. |
| A013 | Protocol/client/CLI | Provider/line stacks; DIRECT session/output/client; removed daemon loopback source | PARTIAL | Active Cancel, aggregate framing and child cleanup remain deficient. R019 removed daemon TCP/token proxy. CLI/provider-client compatibility and wave4/test pairing remain. | One capped framed owned-child exchange; remove remaining CLI/provider compatibility and execute platform cases. |
| A014 | Product depth | Orientation/comparison/documents; optional workers/overlay; recipe fixture | PARTIAL | User tasks retained; fixed recipe registry removed. Deleted scaffolds are not capabilities. | Concrete producer/consumer decisions and quality checks; baseline Search does not close these requirements. |
| A015 | Tests/governance code | xtask/ledger; structural/lifecycle/snapshot/query/provider removals; mixed migration tests | PARTIAL | Obsolete proof systems and loopback-local fixtures were removed. Mixed migration tests still combine real I/O and source assertions. | Preserve causal cases while removing remaining structural fixtures with their owners. |
| A016 | Donors | redb, Qdrant, Aho-Corasick, Git/ripgrep/ignore, Vec; R020 Cargo target/locked semantics | PARTIAL | Concrete seams only; no whole-donor or upgrade claim. Cargo documentation is not evidence of a command having run. | Revisit only for selected implementation questions. |
| A017 | Qualification tree | Complete tree and representative reports/consumers | CHECKED | Archived in Git; only policy/blocker summary remains. | Runtime ceremony A022. |
| A018 | Auxiliary docs | Package/binary filename sweep and supplements | CHECKED for filename/disposition | Duplicate wave/profile/DIRECT specs removed; distinct diagnostics retained. Retention directory now holds notes only, preserving existing links. | Semantic consistency A001. |
| A019 | Issues/PR handoff | #352/#214/#244/#275/#279/#280/#293/#302/#332; R020 PR and #214 | PARTIAL | R020 found PR text stopped at R017 and #214 at R018 despite registry R019. Update their current state without repeating implementation algorithms. | Other issue consistency; no whole-queue claim. |
| A020 | Runtime evidence | Windows, blocked I/O, live Qdrant, relevance/scale, reader growth, restart/restore, child/GC/archive | RUNTIME-ONLY | Deleted fixture claims are not evidence. Static findings define required runs. | Implement/run exact SHA/commands/raw outcomes. |
| A021 | Compiler baseline | Recorded Ubuntu Rust 1.98 runs through 39872058276; R020 tool attempts | PARTIAL | Last previously verified run covers bridge lib at 7d92fb8b; earlier broad daemon check failed. R020 metadata/Clippy could not start because cargo is absent (exit 127). That is an environment failure, not compilation of this checkout. | Locked metadata plus daemon binary/current consumers and selected features; Windows separately. |
| A022 | Runtime qualification/profile identity | Qdrant gate; lexical fixture identity; materializer analogue | PARTIAL | Qdrant gate/process fixtures gone. Lexical fixture-backed activation and profile/test coupling remain. | Remove ceremony through real callers; retain causal examples as tests. |
| A023 | Local GC/storage diagnostics | Legacy maintenance/status/query callers; removed generic retention source | PARTIAL | Generic sweep/purge source and Cargo shell gone; typed stopped-root collector and query-time inventory removal unimplemented. | Implement mark/sweep and remove recursive query/output inventory. |
| A024 | Backup/restore/import | Generic retention/restore removed; #243/#244; R020 local implementation notes | PARTIAL | Target is complete archive plus fresh destination/Qdrant generation. Empty package removal is complete, not delivery of restore. | Concrete implementation and partial/corrupt/interrupted/protected cases. |
| A025 | Feature/harness topology | Root/daemon/CLI manifests; wave ladder; standalone/proof targets; daemon loopback feature | PARTIAL | R020 removes wave7-lifecycle, full-baseline, empty retention manifests/lib and matching lock entries. Wave1–wave6, CLI compatibility and old production modules remain. | Actual current-branch consumers before further cuts; metadata/check/Clippy. |

## What remains in the static audit

- Trace the remaining `wave1`–`wave6`, CLI/provider-client, shared sealed/migration and query consumers on the
  current branch. Default-branch search is discovery, not proof that a current caller is absent.
- Reconcile surviving README/AGENTS/FUNCTIONS and affected issue bodies against actual code and current recipes.
  Verify that useful old requirements survived; remove duplicate rules rather than make another specification.
- Finish lexical/materializer fixture-activation and configuration-authority caller inspection; separate real
  transformation semantics from test data and execution policy.
- Extract useful I/O behavior from mixed migration tests without treating source-text assertions or old phases
  as requirements. Inspect residual structural consumers rather than deleting by filename alone.
- Resolve concrete optional producer/donor seams for exact/navigation/comparison/documents/models/MCP. The current
  baseline does not establish their implementation or quality.

These are unresolved inspection/design questions. Known defects awaiting code below are not newly unread areas.

## Implementation and execution remaining

1. Verify this Cargo cut with locked metadata and the actual daemon binary/current consumers, not only its small
   library. Then collapse remaining wave features with their real consumers; no replacement acceptance profile.
2. Connect one entrypoint, retained source/object/current row and preparation. Implement documented source,
   empty/gap, budget, stale-completion and lost-wakeup corrections; coordinate only actual #598 overlap.
3. Connect one Qdrant runtime/publication path, immutable retrieval state, pending effects, late-writer handling,
   atomic evidence/view commit, rebuild and cleanup.
4. Connect direct scoped query/evidence, one QueryWindow, framed terminal, active Cancel and owned-child cleanup;
   remove legacy CLI/provider/proof/catalogue paths with those consumers.
5. Implement current-format stopped-root GC/archive/import and remove remaining query-time storage inventories.
6. Remove remaining runtime qualification and obsolete fixture consumers; deliver selected product-depth features
   through actual producers. Runtime quality/scale/platform/crash claims wait for the code they exercise.

## Immutable detailed evidence

Earlier register rewrites condensed R001–R018. These links preserve their inspected paths and evidence boundaries;
current implementation recipes still govern the rewrite, not historical instructions.

| Pass | Recorded checkpoint |
|---|---|
| R001–R012 | [Detailed register at 44d7416c](https://github.com/UnknownAlienHuman/eliot-search/blob/44d7416c46554456f16ccd2b3943657b755304ee/docs/audit/ANALYSIS_REGISTRY.md) |
| R013 | [2b37f5ef](https://github.com/UnknownAlienHuman/eliot-search/blob/2b37f5ef1e8cc755bbcd8c0e1f7efc840eff8cf3/docs/audit/ANALYSIS_REGISTRY.md) |
| R014 | [8cf331a4](https://github.com/UnknownAlienHuman/eliot-search/blob/8cf331a44decded2c84c7f579dcbf8434e040dc7/docs/audit/ANALYSIS_REGISTRY.md) |
| R015 | [bf2bde29](https://github.com/UnknownAlienHuman/eliot-search/blob/bf2bde299b3f009527397571bd9baeea51fde840/docs/audit/ANALYSIS_REGISTRY.md) |
| R016 | [53ab0ea3](https://github.com/UnknownAlienHuman/eliot-search/blob/53ab0ea33f8d405f72e984881b5aea0db69e7c3b/docs/audit/ANALYSIS_REGISTRY.md) |
| R017 | [15145667](https://github.com/UnknownAlienHuman/eliot-search/blob/151456672ea4ec8773ed65c5017cb63989912d30/docs/audit/ANALYSIS_REGISTRY.md) |
| R018 | [174f3ec7](https://github.com/UnknownAlienHuman/eliot-search/blob/174f3ec758cb0ec5c38b780cd3de913be9120fdc/docs/audit/ANALYSIS_REGISTRY.md) |
| R019 | [156528ad](https://github.com/UnknownAlienHuman/eliot-search/blob/156528ad0a2ab64599d9cef01d3eeea2b6f11040/docs/audit/ANALYSIS_REGISTRY.md) |

## Pass log

### R001–R014

Removed parallel governance/specification systems, xtask/ledger, 66 structural tests and stale qualification;
audited Qdrant/lexical, source currentness, preparation/publication, evidence/window/transport and GC/restore.
The detailed checkpoints above retain exact inspected paths and execution limitations.

### R015 — structural/lifecycle proof consumers

Removed ten source-text tests and five restore/retention/purge proof suites (123,096 source bytes).

### R016 — isolated snapshot/sealed applications

Removed eight explicit snapshot/sealed applications, private snapshot/control/local-BM25 source and structural
isolation test (5,765 lines).

### R017 — restore and generic lifecycle implementation

Removed daemon restore source and generic retention/purge/sweep/restore code (3,218 production Rust lines),
leaving the temporary manifest/lock shell removed by R020.

### R018 — disconnected query/provider/rebuild fixtures

Removed nine process proofs (6,740 lines / 243,346 bytes): in-memory query oracle, manual DIRECT/Qdrant join,
rebuild/pin/reclaimer, stale qualification, pairing/provider loopback, recipe registry and random handles. Moved
causal cases to current owner map.

### R019 — remove daemon TCP/token loopback owner (2026-10-23)

Inspected exact head `174f3ec758cb0ec5c38b780cd3de913be9120fdc`:

- daemon `entry.rs` and `Cargo.toml`;
- `authenticated_proxy.rs`, kernel facade and all 11 kernel files;
- `endpoint.rs`, kernel facade and all seven endpoint kernel files;
- `proxy_child.rs` and seven child files;
- `proxy_exchange.rs` and five exchange files;
- provider-composition cfg map;
- current daemon test directory and caller searches after R018;
- protocol/CLI owner docs and issues.

Confirmed:

1. `authenticated_proxy` is reachable only through the feature-gated interceptor in `entry.rs`.
2. The endpoint/proxy chain implements loopback TCP, token/pairing proofs, replay envelopes, child forwarding and
   multi-line terminal parsing selected for deletion.
3. After R018, no current integration test path names/imports `endpoint` or `authenticated_proxy`; embedded tests
   live inside the owner itself and prove only that obsolete transport.
4. General child lifetime/partial-output/cancellation cases remain required but belong to the new owned-child
   framed protocol, not this TCP/token stack.
5. `provider_composition` still referenced `legacy-loopback-harness` in cfg clauses; removing the feature requires
   narrowing those clauses to test/wave4 only to avoid an undefined-feature cfg.
6. CLI/provider-client loopback compatibility is separate and remains; R019 does not silently delete it or claim
   the selected new protocol exists.

R019 removes the daemon feature, interceptor, authenticated proxy, endpoint, proxy-child and proxy-exchange source
(36 files) and narrows provider cfgs. It changes production Rust/Cargo but no dependency or lockfile. No workflow
run or compiler/Clippy/runtime result was recorded; static caller inspection is not build evidence.

### R020 — empty Cargo-shell removal and evidence reconciliation (2026-10-23)

Inspected source head `156528ad0a2ab64599d9cef01d3eeea2b6f11040`:

- root `AGENTS.md`, architecture, `README.md`, `QUICKSTART.md`, `Cargo.toml`, complete `Cargo.lock`;
- `bins/eliot-searchd/{AGENTS.md,Cargo.toml}`;
- `crates/search-runtime/search-retention/{Cargo.toml,README.md,AGENTS.md,FUNCTIONS.md,src/lib.rs}`;
- `.github/workflows/manual-workspace-check.yml`, current register, PR #610, #214 and intervening R016–R019 commits;
- exact GitHub diff of the prepared Cargo cut against this head.

Findings and edits:

- The no-API retention shell still participated in workspace, daemon feature/dependency and lock graphs. Removed
  its manifest/library, root member/dependency, daemon dependency and wave7/full-baseline aliases together.
- Lockfile adjustment is an exact manual prune, not Cargo regeneration: remove one daemon dependency edge and
  the corresponding 13-line path-package block. Reviewed GitHub diff shows no third-party version/checksum or
  unrelated dependency change. Resolver confirmation remains pending.
- Retention's directory remains documentation-only to preserve existing recipe links. README/AGENTS/FUNCTIONS
  distinguish removed generic code from the still-unimplemented concrete GC/archive/import. No replacement crate.
- Root README/QUICKSTART still described deleted loopback/full-baseline and promised a runnable fresh checkout.
  Corrected removed surfaces and made current-build/run uncertainty explicit. Compiler examples now select the
  actual daemon binary; a library-only check is not evidence for its composition.
- Root Cargo still demanded a qualified client receipt despite the earlier gate removal; corrected that comment
  without changing the SDK pin. Removed daemon Cargo's reference to deleted SOURCE_MAPPING.md.
- PR/#214 lagged the live registry. Their handoff is synchronized with this cut. Restored immutable links to
  earlier detailed register versions rather than erasing audit provenance during condensation.

Execution boundary: local Cargo/rustc were unavailable. Attempts to start pinned metadata and daemon Clippy
returned `cargo: command not found` (exit 127), before checkout compilation. Git network access also failed DNS.
The loaded GitHub connector permits repository writes but exposes no new workflow-dispatch action; rerunning an
old job would not check this head. No automatic trigger, dependency upgrade, private #598 edit, test run or passing
compiler claim was introduced. The official [Cargo check reference](https://doc.rust-lang.org/cargo/commands/cargo-check.html)
was consulted only for target and locked-mode semantics, not as execution evidence.

Required next execution on the published cut: locked metadata, pinned daemon-binary check and strict Clippy,
then its actual selected consumers/features. This narrow removal is not a full current-branch caller audit,
feature-ladder completion, working maintenance implementation or product readiness.
