# Analysis registry

Current checkpoint and evidence, not another specification or permission system. Algorithms live with owners.

<a id="resume"></a>
<a id="resume-after-interruption"></a>
## Resume

**Checkpoint:** R055, 2026-10-28 — reconcile instructions and shorten implementation entrypoints.
**Branch:** codex/refactor-preparation-20261017, [PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610).
**Inspected parent:** 45ddf887c9e84861b1302334a1f5ecabb263f7f3 (R054).
**Original source baseline:** c8e38b6fc9829f1cd38070773b1b3e5b21de18ad.

R055 is documentation-only. No product code, manifest, lockfile or workflow is changed. The target product remains
unconnected end to end. Until PR #610 merges, main retains the older documentation generation.

Next implementation: [Foundation](../implementation/README.md#1-foundation--invocation-settings-and-root).
Read its affected settings/root recipes and actual callers, not the whole audit history. Implement the small
root/object platform seam, then root/settings and the one-file path; do not start another broad audit.

<a id="established-decisions--do-not-rediscover"></a>
## Decisions retained and clarified

| Question | Decision and owner |
|---|---|
| Query k/b versus index identity | Query-time settings never cause physical rebuild; [lexical](../../crates/search-lexical/FUNCTIONS.md) and corrected [projection](../../crates/search-index-qdrant/search-projection-planner/FUNCTIONS.md). |
| Interrupted initialization | Strict recognized-empty discard/retry, no resume/journal; [root](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md). |
| Index cleanup | Whole-generation only, including conditional Stage 3B; [publication](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md#view-lifetime-and-whole-generation-cleanup). |
| Context before a query exists | Execution-only deadline/cancellation/counters; query adds its view/scope/scoring; [shared boundary](../implementation/README.md#minimal-cross-owner-values). |
| Windows finalization | One candidate recipe at the root owner; exact Q&A environment/source in donor register; not an executed safety guarantee. |
| Documentation maintenance | Owner defines rule once; README/AGENTS/index link to it. Historical readsets remain at immutable Git links. |

The selected Qdrant upgrade, complete-generation reference implementation and conditional optimization order are
unchanged. Optional encryption/models/documents/adapters do not block the lexical product.

<a id="coverage-ledger"></a>
## Documentation work

| Stage | R055 disposition | Continue with |
|---|---|---|
| Conflicting rules | Corrected query/index profile split, initialization, cleanup and context ownership | Change the owner first when behavior changes. |
| Entry/navigation | Shortened root/index/implementation and CLI/daemon entrypoints; restored owner map and meaningful legacy registry anchors | Check incoming links when moving a section. |
| Duplicate recipes | Removed Windows recipe copies from entry/object/composition documents; root owns it | Apply the same owner/link rule to other families when touched. |
| Evidence/history | Exact Windows source/environment added; old detailed records linked, not copied | Keep inspected and executed states distinct. |
| Whole-tree assurance | No claim that every remaining owner document or code path is now defect-free | Review only the next affected owner/caller chain; record concrete new contradictions. |

This is a bounded documentation repair, not repository-wide code audit closure.

<a id="delivery-remaining"></a>
## Remaining product work

Foundation and the one-file Qdrant/CLI path remain to implement. Restart/inventory/convergence, measured corpus
budgets, Exact/session and selected product depth remain afterward. The
[delivery sequence](../implementation/README.md#delivery-sequence) is the single owner of their order and scope.
R055 does not mark a product stage complete or make optional work a prerequisite.

<a id="evidence-status"></a>
<a id="execution-evidence"></a>
## Evidence status

| Area | Evidence / limit |
|---|---|
| R055 documentation | Four contradictions addressed in their owners and affected instructions; entrypoints/known broken navigation reconciled. This is editorial/static evidence. |
| Windows references | Microsoft API pages and dated Q&A reproduction inspected 2026-10-28; no ELIOT Windows canary executed. |
| Runtime safety | Windows namespace/ancestor/reparse handling and durability remain unverified; the candidate must refuse unsupported profiles. |
| Qdrant | Lock remains 1.19.0; selected 1.20.x integration has not been compiled or executed. |
| Compiler history | R031 materializer/unitizer check + minimal Clippy, [run 40084893130](https://github.com/UnknownAlienHuman/eliot-search/actions/runs/40084893130); not a new whole-product build. |
| Current product | Foundation, current binaries, end-to-end search, restart, relevance and scale remain NOT_RUN. |
| R055 Rust/tests | Not run: documentation-only changes; cargo is absent in this editing environment. No new test suite or workflow added. |
| Link-check scope | Known navigation targets/anchors are the repair scope; no new repository-wide 202-file crawler result is claimed. |

<a id="evidence-index"></a>
## Prior evidence

- [R054 registry and earlier checkpoint list](https://github.com/UnknownAlienHuman/eliot-search/blob/45ddf887c9e84861b1302334a1f5ecabb263f7f3/docs/audit/ANALYSIS_REGISTRY.md).
- [Detailed earlier registry through R031](https://github.com/UnknownAlienHuman/eliot-search/blob/da382b66e59d0b44e758bad9b22e2f5703a4e8c2/docs/audit/ANALYSIS_REGISTRY.md).
- [Current donor evidence](DONOR_VERIFICATION_REGISTER_2026-10-09.md).
- CLI/daemon/ports recipes link their own immutable prior source readsets.

## Updating this record

Keep one short current checkpoint with inspected SHA, changed owners, actual checks and unrun work. Preserve old
readsets through immutable links. Do not copy local algorithms, add approval tokens or accumulate a second job log.
