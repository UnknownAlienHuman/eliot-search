# Work register

One place for remaining work, current evidence and the next task. Algorithms stay in owner recipes.
**R067 — repository cleanup and executable work register, 2026-10-30.** Base: `f3d710131aa3b06a63198694bed7ec82b1e251df`.

<a id="resume"></a>
<a id="resume-after-interruption"></a>
## Next task

**F1 is next.** Start from [actual entry.rs dispatch](../implementation/CODE_MAP.md#existing-dispatch-to-replace),
not old src/main.rs harnesses. Capture invocation once; help/version and rejected input have no effects; connect
plain root commands without earlier legacy interception. Do not start all rows or optional features at once.

One manager owns one worktree and the integration diff. Give an executor one row plus its affected paths,
owner recipe, preserved invariants and stop condition. Read its changed caller path, not every document.
After two failed approaches, record the cause and change approach; do not stack fallback wrappers or suppress errors.

<a id="work-register"></a>
<a id="delivery-remaining"></a>
## Remaining work

READY means the next connected implementation can start; WAIT means its listed prerequisite is not complete.
DONE requires landed files and recorded checks, never a revised recipe. IDs are local navigation, not a new API.
The [package register](WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md#package-register) covers every remaining member.

| ID / status | Depends on | Exact starting paths and owner | Stop condition / removal |
|---|---|---|---|
| C0 / DONE in this change | None | Old xtask/swarm/tooling and five unconsumed packages | Source, manifests, unused lock entries, exclusive fixtures/workflows and competing documents removed together; no surviving input reader lost. |
| F1 / READY | None | bins/eliot-search{,d}/src/entry.rs; daemon config_composition; [settings](../../crates/search-config/FUNCTIONS.md) | One parsed command reaches the real handler; help/version/invalid input avoid store/spawn effects. Remove converted maybe_run/fallback routes. Include real binaries in check/Clippy. |
| F2 / WAIT | F1 | daemon owner_composition, native_root_command; [root](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md) | Plain inspect/init/open and factual status use one lifetime guard. Move concrete OS/file operations, remove legacy key/epoch/initialization protocols; do not add Linux no-op security adapters. |
| S1 / WAIT | F2 | owner_composition/kernel/native_source.rs, safe_reader_adapter; [source map](../../crates/search-source/README.md#ownership) | Same acquired bytes reach plain ObjectRef and guarded current row. Delete receipt/provenance/registry replay and duplicate writers with their actual consumers. |
| P1 / WAIT | S1 | preparation_composition; [materializer](../../crates/search-prep/search-materializer/FUNCTIONS.md), [unitizer](../../crates/search-prep/search-unitizer/FUNCTIONS.md) | One UTF-8 representation/ranges, one stored outcome, no adjacent reparse or second writer. Empty/gap and stale-result semantics remain truthful. |
| Q1 / WAIT | P1 | publication_composition, search-control-redb, [bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md), [publisher](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) | Real fixed build, matching current/evidence state and installed view. Adopt exact client/server with consumers; remove supervisor/lease/proof coupling, not by creating dummy credentials. |
| U1 / WAIT | Q1 | public_client, public_runtime_service, provider_composition; [CLI](../../bins/eliot-search/FUNCTIONS.md), [client](../../crates/search-provider-client/FUNCTIONS.md), [protocol](../../crates/search-provider-protocol/FUNCTIONS.md) | One-shot CLI returns a verified retained excerpt via final child-stdio path. Remove pairing/pipe/bootstrap/duplicate requests and old query plan/executor wrappers. No throwaway transport. |
| R1 / WAIT | U1 | source_roots, source-reconcile, control, publication, pins/reclaimer | Restart and complete desired-state comparison converge without lost wakeups; old views stay valid and whole generations clean safely. Remove duplicate journals, point GC and full-catalogue status scans. |
| X1 / WAIT | R1 | query family, daemon continuation/result_handles, client | Exact scans retained inventory; live windows keep original view/order, bounded expansion and expiry. Delete duplicate handle/catalogue/fence/replay state. |
| M1 / WAIT | R1 | daemon maintenance/restore_composition; [maintenance](../../crates/search-runtime/search-retention/FUNCTIONS.md) | Complete stopped-root marking, non-runnable archives and isolated fresh-destination import. Remove generic lease/purge/restore machinery, preserving originals. |
| O1 / DEFERRED | Connected baseline + explicit need | code-enricher, overlay, comparator, subject-resolver; [optional depth](../product/AGENT_ANALYSIS_FRAMEWORK.md#optional-depth) | Remove unconsumed scaffolds after feature/launch/caller check. Implement optional behavior only for a selected real consumer; no empty replacement binaries or placeholders. |
| V1 / WAIT | Connected shipped paths | [later checks](../implementation/TEST_CUTOVER.md), issue #215 | Run actual behavior/platform/relevance/resource cases; publish only supported measured claims and executed Quickstart. Before this, production cuts run minimal check/Clippy, not broad tests. |

Stage 3B is conditional on declared Stage 3A budgets failing, not an additional prerequisite. Delivery order and
mechanisms remain in the [implementation sequence](../implementation/README.md#delivery-sequence). Shared
contracts/domain/ports shrink with each real consumer above; there is no separate global rewrite phase.

<a id="established-decisions--do-not-rediscover"></a>
<a id="decisions-and-owners"></a>
## Preserved decisions

[Local scope](../architecture/LOCAL_SCOPE.md): plain per-account storage, same-host operator-owned Qdrant, owned-child
stdio. No application encryption, key platform, grants or multi-user layer. Keep source integrity, exclusions,
finite work, coherent views and conservative deletion. Preserve user originals/keys and unpublished branches.

<a id="coverage-ledger"></a>
<a id="changed-documentation"></a>
## Cleanup delivered

Removed old task/receipt generators and their complete repository input closure: xtask, swarm packets, obsolete
configuration catalogues, qualification reports, PowerShell wrappers, redundant workflows, old master/contracts
and superseded audits. Historical evidence is accessible at fixed Git revisions, not duplicated in active context.
Removed unconsumed revision crypto, disconnected Eliot adapter and refusal-only model/document workers.
No live source was replaced by a stub; remaining coupled code is mapped above, not called deleted.

The single manual workflow checks a selected production package/target/features and Clippy without requiring
workspace-wide tests. The package register maps every remaining explicit Cargo member to one of these cuts.

<a id="evidence-status"></a>
<a id="execution-evidence"></a>
## Verification and known blockers

Full snapshot was obtained from Actions at `15d2a25b` (base plus temporary read-only snapshot workflow).
All 2,735 tracked blobs were verified against that exact tree, including 572 original Markdown files.
Review covers the full path inventory, manifest/launch references for removed components and retained-document
navigation. It is not a proof of every remaining runtime algorithm or an independent-agent review.

**Existing baseline problems, not cleanup regressions:**

| Area / exact baseline | Observed result | Required follow-through |
|---|---|---|
| Windows default CLI + daemon, Rust 1.98 | cargo check passes with warnings | This does not establish new product behavior or alternate features. |
| Linux default daemon | 19 compiler errors: missing platform/unsupported module paths in sealed_owner_epoch, sealed_store and sealed_transaction; unconditional Windows-secret imports in owner_composition; missing File import in direct_store/store.rs | F1/F2 must resolve the actual call/target boundary; no cfg-hidden success claim or fake non-Windows encryption. |
| Strict Clippy | Stops in search-os-secrets/src/legacy_revision_protection/codec.rs:59 on needless_lifetimes | Remove/repair the actual legacy dependency closure; no global allow or guessed clean result beyond the first failing package. |

The baseline diagnostics are in [run 41152732245](https://github.com/UnknownAlienHuman/eliot-search/actions/runs/41152732245).
Its first diagnostic shell stopped at the first failure; only commands with recorded logs count. Candidate
base/head comparison and final navigation results are recorded in this change's PR before merge.
No product tests, full-feature build, Qdrant run, performance result or autonomous-agent completion is claimed.

## Update and finish a task

Change status only with the actual result: record commit, command/target/features, outcome and remaining blocker.
Update changed package rows and one owning recipe; keep issue bodies as links. A blocked agent leaves its concrete
cause and safe diff, not fabricated completion. The integration owner rechecks dispatch, removed imports and the
minimal build closure before landing. Remaining source defects are not erased by a smaller file count.

<a id="evidence-index"></a>
<a id="prior-evidence"></a>
## History

[R066 registry](https://github.com/UnknownAlienHuman/eliot-search/blob/f3d710131aa3b06a63198694bed7ec82b1e251df/docs/audit/ANALYSIS_REGISTRY.md)
retains earlier evidence. The old #610 branch is reference material to reconcile, not a bulk merge instruction.
