# Work register

One place for remaining work, current evidence and the next task. Algorithms stay in owner recipes.
**R069 — connected daemon invocation and shadow-route removal, 2026-11-02.** Base: `d7ec099e4a7cf107c190ee6b124cc7cd496bfeee`.

<a id="resume"></a>
<a id="resume-after-interruption"></a>
## Next task

**Finish consumed bootstrap settings, then plain root ownership.** CLI and daemon now each capture native argv
once and select the real handler. Do not redo dispatch. Start in daemon config_composition and its actual
NativeRootRequest/root consumer: capture startup cwd, merge only consumed bootstrap fields and pass checked
values directly. Remove raw-argv custody and the displaced config registry with their consumers, not by adding
a second configuration facade. A small F1 settings change may land together with its F2 root consumer; no
placeholder settings layer is required to cross the row boundary.

The [code map](../implementation/CODE_MAP.md#existing-dispatch-to-replace) shows actual routes. Native-root and
preparation handlers currently reject config flags they do not consume. Existing DIRECT effects, protected-root
mechanisms and the final serviced client still need their named cutovers. Do not implement missing Unix crypto
or enable every old feature to make the new plain product appear complete.

One manager owns one worktree and the integration diff. Give an executor one row plus its affected paths,
owner recipe, preserved invariants and stop condition. Read its changed caller path, not every document.
After two failed approaches, record the cause and change approach; do not stack fallback wrappers or suppress errors.

<a id="work-register"></a>
<a id="delivery-remaining"></a>
## Remaining work

READY means work can start; PARTIAL means a named increment landed and the remainder can continue; WAIT means
its prerequisite is not complete.
DONE requires landed files and recorded checks, never a revised recipe. IDs are local navigation, not a new API.
The [package register](WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md#package-register) covers every remaining member.

| ID / status | Depends on | Exact starting paths and owner | Stop condition / removal |
|---|---|---|---|
| C0 / DONE (R067) | None | Old xtask/swarm/tooling and five unconsumed packages | Source, manifests, unused lock entries, exclusive fixtures/workflows and competing documents removed together; no surviving input reader lost. |
| F1 / PARTIAL | None | daemon src/entry.rs, app, config_composition; [settings](../../crates/search-config/FUNCTIONS.md) | Both installed entrypoints capture once and route directly. Finish consumed typed bootstrap settings with the real root constructor; eliminate residual raw-argv/config revalidation. Keep help/version/rejected invocation effect-free and check the changed binaries. |
| F2 / WAIT for consumed settings | F1 settings, may land together | daemon owner_composition, native_root_command; [root](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md) | Plain inspect/init/open and factual status use one lifetime guard. Move concrete OS/file operations, remove legacy key/epoch/initialization protocols; do not add Linux no-op security adapters. |
| S1 / WAIT | F2 | owner_composition/kernel/native_source.rs, safe_reader_adapter; [source map](../../crates/search-source/README.md#ownership) | Same acquired bytes reach plain ObjectRef and guarded current row. Delete receipt/provenance/registry replay and duplicate writers with their actual consumers. |
| P1 / WAIT | S1 | preparation_composition; [materializer](../../crates/search-prep/search-materializer/FUNCTIONS.md), [unitizer](../../crates/search-prep/search-unitizer/FUNCTIONS.md) | One UTF-8 representation/ranges, one stored outcome, no adjacent reparse or second writer. Empty/gap and stale-result semantics remain truthful. |
| Q1 / WAIT | P1 | publication_composition, search-control-redb, [bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md), [publisher](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) | Real fixed build, matching current/evidence state and installed view. Adopt exact client/server with consumers; remove supervisor/lease/proof coupling, not by creating dummy credentials. |
| U1 / WAIT | Q1 | public_client, owner_composition direct_registry, provider_composition; [CLI](../../bins/eliot-search/FUNCTIONS.md), [client](../../crates/search-provider-client/FUNCTIONS.md), [protocol](../../crates/search-provider-protocol/FUNCTIONS.md) | One-shot CLI returns a verified retained excerpt via final child-stdio path. Remove pairing/pipe/bootstrap/duplicate requests and old query plan/executor wrappers. No throwaway transport. |
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

R069 connects the daemon app dispatcher and passes captured arguments/configuration to existing handlers.
One help/version path replaces competing help. Native file paths remain lossless; invalid command/query text
and unexpected shape refuse before their effects. No new crate, dependency, state service or transport.

Removed public_runtime_service and secure_commands: their persistent commands were already handled by the earlier
DIRECT-registry owner. Their exclusive fixtures and four source-shape test files leave with the removed topology.
The DIRECT effect implementation is retained, not replaced by a stub. Legacy refusal remains before writes.
R067 cleanup and R068 CLI remain landed; Cargo graph, other branches and user bytes/keys are unchanged.

<a id="evidence-status"></a>
<a id="execution-evidence"></a>
## Verification and known blockers

[Run 41519190759](https://github.com/UnknownAlienHuman/eliot-search/actions/runs/41519190759) compared exact base
`d7ec099e` and source `3ecb5837` on Ubuntu 24.04 and Windows 2025, Rust 1.98.0. Actual installed daemon target,
default and explicit `--features legacy-loopback-harness`:

```text
cargo +1.98.0 check --locked -p eliot-searchd --bin eliot-searchd [feature selection]
cargo +1.98.0 clippy --locked -p eliot-searchd --bin eliot-searchd [feature selection] --no-deps -- -D warnings
```

| Check | Base → changed source | Required follow-through |
|---|---|---|
| Windows check, default/harness | PASS → PASS, warnings remain | Compilation, not executed command or product behavior. |
| Linux check, default/harness | FAIL → FAIL, same 19 compiler errors | Existing missing unsupported-module paths, unconditional Windows imports and missing File import remain with the actual platform/root cut. No fake adapter or cfg-hidden success. |
| Windows strict package Clippy, default | FAIL: 164 → 390 diagnostics | Removing shadow callers exposes unused support; not an unchanged or passing lint result. |
| Windows strict package Clippy, harness | FAIL: 190 → 414 diagnostics | Same exposure, plus inherited harness debt. |
| Linux strict package Clippy | FAIL at the existing compiler boundary | No clean lint claim for code not reached. |

New Windows diagnostic blocks are unused imports/declarations/fields in the remaining legacy support, not a
reason to retain the fake calling routes or add lint suppressions. Trace/prune continuation/result_handles,
service_output, old store reads and native registration/read/sync support with their actual feature/harness
consumers during F1/F2/U1/X1. Their source was previously kept apparently live by unreachable dispatchers.
The retained source is not declared clean. `--no-deps` does not qualify dependency lints.

Recovered the complete 1,709-blob snapshot at `c1937655` (base plus temporary workflow) and verified every blob.
Reviewed effective route precedence, source/module/launch references and removed-family consumers. Exact authored
outputs were hash-checked before creating the candidate; no source was synthesized by the temporary transfer job.
Final navigation, path and diff results are recorded in this change's PR, not inferred from earlier checks.
Temporary transfer/verification workflows are removed before landing. No tests, executed daemon scenario,
full-feature build, Qdrant, runtime/performance result or independent-agent review is claimed.

## Update and finish a task

Change status only with the actual result: record commit, command/target/features, outcome and remaining blocker.
Update changed package rows and one owning recipe; keep issue bodies as links. A blocked agent leaves its concrete
cause and safe diff, not fabricated completion. The integration owner rechecks dispatch, removed imports and the
minimal build closure before landing. Remaining source defects are not erased by a smaller file count.

<a id="evidence-index"></a>
<a id="prior-evidence"></a>
## History

[R068 registry](https://github.com/UnknownAlienHuman/eliot-search/blob/d7ec099e4a7cf107c190ee6b124cc7cd496bfeee/docs/audit/ANALYSIS_REGISTRY.md)
retains earlier evidence. The old #610 branch is reference material to reconcile, not a bulk merge instruction.
