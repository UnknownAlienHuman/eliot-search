# Work register

One place for remaining work, current evidence and the next task. Algorithms stay in owner recipes.
**R068 — connected CLI invocation increment, 2026-10-31.** Base: `26497dfd7319e22b6faf4f28e15b9695bf007c18`.

<a id="resume"></a>
<a id="resume-after-interruption"></a>
## Next task

**Finish F1 in daemon/settings.** The CLI now captures native arguments once and routes directly to current
handlers. Do not redo that change or call all of F1 complete. Start at the daemon's
[actual entry.rs dispatch](../implementation/CODE_MAP.md#existing-dispatch-to-replace) and config_composition:
replace repeated argument reads/interceptors with one selected command and consumed settings. Help/version and
invalid invocation must finish before effects. Plain root ownership remains F2; the final serviced client is U1.
Do not start all rows, rebuild removed copies or substitute the old src/main.rs test harness.

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
| F1 / PARTIAL | None | daemon src/entry.rs, app, config_composition; [settings](../../crates/search-config/FUNCTIONS.md) | CLI capture/direct routing is connected. Finish daemon routing and actually consumed settings; remove converted interceptors. Do not claim complete request validation or final transport from this CLI-only cut. Include changed binaries in check/Clippy. |
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

R068 changes six connected CLI source files and removes five unlinked copies: directory_client.rs, help.rs,
owned_client.rs, paged_client.rs and service_client.rs (1,625 lines). Capture and select the command once;
help/version have one owner; existing forwarding borrows the original arguments. No new crate, trait, dependency,
protocol or test suite. Original user files, unrelated branches and daemon behavior are unchanged.

R067's five-package/tooling removal and 47-member register remain intact. Its exact inventory, Cargo normalization
and older platform checks are retained in the [previous registry](https://github.com/UnknownAlienHuman/eliot-search/blob/26497dfd7319e22b6faf4f28e15b9695bf007c18/docs/audit/ANALYSIS_REGISTRY.md).

<a id="evidence-status"></a>
<a id="execution-evidence"></a>
## Verification and known blockers

[Run 41191228363](https://github.com/UnknownAlienHuman/eliot-search/actions/runs/41191228363) compares base `26497dfd`
and source `881548f1` on Ubuntu 24.04 and Windows 2025, Rust 1.98.0. It checks the actual installed CLI target,
not the old test harness, both with default features and with `--features legacy-loopback-harness`:

```text
cargo +1.98.0 check --locked -p eliot-search --bin eliot-search [feature selection]
cargo +1.98.0 clippy --locked -p eliot-search --bin eliot-search [feature selection] --no-deps -- -D warnings
```

| Check | Base and source result | Meaning |
|---|---|---|
| CLI check, both platforms / both selections | PASS with existing warnings | Connected invocation and retained harness compile; no daemon or end-to-end qualification. |
| CLI strict package Clippy, Linux | FAIL: 13 default / 12 harness diagnostics | Same messages, locations and suggestions before/after; no new diagnostic. |
| CLI strict package Clippy, Windows | FAIL: 13 default / 13 harness diagnostics | Same messages, locations and suggestions before/after; no new diagnostic. |

`--no-deps` checks this package, not dependency lints. Remaining diagnostics concern the old native credential/
registration/provider client: unused exports/variants, redundant visibility, two const-function suggestions,
two redundant continues and a Windows borrowed-value suggestion. Keep them assigned to the actual consumer
cutover; they are not PASS or grounds for blanket suppression. The daemon is unchanged and not rechecked here:
R067's 19 Linux compiler errors and dependency-level strict Clippy failures remain F1/F2/U1 work.

The complete candidate archive contains 1,709 tracked blobs; each matched its exact Git tree. Local base and
candidate trees matched GitHub; all six edited source outputs matched the local review. Full surviving-tree
search found no module/target/import/script reference to the five deleted copies. One args_os call remains in
installed CLI source; the independent src/main.rs test harness is unchanged.

All 188 retained Markdown files and 714 relative links/fragments resolve. The 101 explicit Cargo target/path
references in 48 workspace manifests exist; YAML and diff whitespace checks pass on the final candidate. External/pinned URLs are not crawled. No independent-agent review, product
tests, executed CLI scenario, daemon check, Qdrant or performance result is claimed. Temporary read-only CI is
removed before landing; final documentation-only changes do not alter the checked source or Cargo graph.

## Update and finish a task

Change status only with the actual result: record commit, command/target/features, outcome and remaining blocker.
Update changed package rows and one owning recipe; keep issue bodies as links. A blocked agent leaves its concrete
cause and safe diff, not fabricated completion. The integration owner rechecks dispatch, removed imports and the
minimal build closure before landing. Remaining source defects are not erased by a smaller file count.

<a id="evidence-index"></a>
<a id="prior-evidence"></a>
## History

[R067 registry](https://github.com/UnknownAlienHuman/eliot-search/blob/26497dfd7319e22b6faf4f28e15b9695bf007c18/docs/audit/ANALYSIS_REGISTRY.md)
retains earlier evidence. The old #610 branch is reference material to reconcile, not a bulk merge instruction.
