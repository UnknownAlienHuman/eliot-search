# Analysis registry

Current checkpoint and evidence only. Algorithms live with their operation owners; this record grants no
implementation permission or runtime readiness.

<a id="resume"></a>
<a id="resume-after-interruption"></a>
## Resume

**Checkpoint:** R058, 2026-10-28 — retire competing legacy documentation entrypoints on main.
**Inspected base:** `d3330da162822e6cfe7b4a5e1d8b91cb8dcc5aad` (R057, merged PR #612).
**Scope:** Markdown only; no source, manifest, lockfile, configuration value, workflow or test-source change.

Current instructions are already published on main. Source refactor
[PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610) is still separate and unmerged; its earlier
source removals are not present merely because its documentation was published. Preserve unpublished #598 work.

Next implementation remains [Foundation](../implementation/README.md#1-foundation--invocation-settings-and-root),
then one retained file through Qdrant to a CLI excerpt. Read that stage, affected owners and actual callers;
do not restart the full design audit or replay an old launch queue.

<a id="established-decisions--do-not-rediscover"></a>
## Current decisions

The [implementation guide](../implementation/README.md#owners) routes all owning rules. R055/R056 decisions remain:
physical/query-profile separation; execution-only lower I/O; recognized-empty init discard/retry; whole-generation
cleanup; pinned query windows with owned pending output; no premature Stage 3B or unused shared DTO families.

Root [AGENTS.md](../../AGENTS.md) owns work/verification/landing. The delivery sequence owns stage order; the root
[Quickstart](../../QUICKSTART.md) owns executable status. Dated process, architecture and status pages cannot
supply a second queue, protocol, readset or prerequisite.

<a id="coverage-ledger"></a>
## Documentation work

Retired six misleading entrypoints: execution protocol, completion DAG, implementation status, agent launch gate,
duplicate Quickstart and P00 pack index. Each now routes directly to a current owner and an immutable copy of its
previous contents. The large 8.4 master and machine-readable legacy inputs are not deleted or rewritten here;
retire them with their consuming tooling, not by weakening a validator or pretending the code cut already landed.

This removes the old ban on documentation merges, frozen-scope forms/quotas, obsolete writer/next-task tables,
eleven-recipe compatibility rule and TCP/token quickstart from those entrypoints. It does not change product code.
Old validators and remaining inner legacy documents are not current acceptance gates; their entire graph was not
requalified by this edit. Historical fragment references belong to the linked original revision.

<a id="delivery-remaining"></a>
## Remaining work

[Delivery sequence](../implementation/README.md#delivery-sequence) remains the implementation-order owner. Reconcile
any source integration with main's instructions and run its smallest compiler/Clippy closure. Do not import the
unqualified accumulated PR #610 diff or wait for whole-product completion to land a coherent source change.

Remaining archival cleanup is bounded by actual consumers: the 8.4 master, inner P00/wave/package-status/coverage inputs and
obsolete tooling still exist on main. Inspect and retire a used path with its caller; do not perform another broad
architecture rewrite or restore its validators to make historical checks pass.

<a id="evidence-status"></a>
<a id="execution-evidence"></a>
## Evidence status

| Check | Result / limit |
|---|---|
| Input | Live main was read at the exact base above. GitHub search still returned older indexed revisions; relevant files were fetched at the pinned main revision. |
| Selected navigation | 21 relative links across the seven changed pages and their named anchors checked against the current destination files. Not a crawl of all retained legacy links or external URLs. |
| Change scope | Seven Markdown paths only. Review the parent/candidate comparison; no source deletion or permanent checker added. |
| Historical preservation | Every retired page links its original path at the immutable inspected base. Existing legacy files are not described as repaired or removed code. |
| Verification boundary | Candidate `git diff --cached --check` and the selected link check passed. Old validator behavior and historical incoming fragment links are not certified; existing external links were not crawled. |
| Compiler/runtime | No build, Clippy or test suite run for this docs-only change. Source presence and old narrow checks are not a current product build. |
| Product | Foundation/current binaries, Windows finalization/durability, selected Qdrant integration, restart, relevance and scale remain unverified. |

<a id="evidence-index"></a>
## Prior evidence

[R057 registry](https://github.com/UnknownAlienHuman/eliot-search/blob/d3330da162822e6cfe7b4a5e1d8b91cb8dcc5aad/docs/audit/ANALYSIS_REGISTRY.md)
preserves the publication checks and earlier source readsets. The
[donor register](DONOR_VERIFICATION_REGISTER_2026-10-09.md) separates upstream evidence from installed/executed behavior.

Keep one current checkpoint. Change algorithms at their owner; update affected links and conflicting issue/PR
status in the same slice. Do not duplicate algorithms or accumulate an audit diary here.
