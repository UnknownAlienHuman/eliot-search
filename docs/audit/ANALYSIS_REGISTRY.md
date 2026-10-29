# Analysis registry

Current checkpoint and evidence. Algorithms remain with their owners.

<a id="resume"></a>
<a id="resume-after-interruption"></a>
## Resume

**Checkpoint:** R061, 2026-10-29 — source/store entrypoints aligned with their owning recipes.
**Inspected main:** `4cb8dc4d61924ee0a1a2f3ce056b7086ac45c332` (R060, merged PR #615).
**Scope:** six Markdown files; no product source, Cargo/lockfile, configuration, workflows or tests changed.

Current instructions are on main. [PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610) is separate
unmerged source work. Preserve unpublished #598 overlap. Next remains
[Foundation](../implementation/README.md#1-foundation--invocation-settings-and-root), then one retained file
through Qdrant to a verified CLI excerpt. Optional encrypted storage does not gate the plain path.

<a id="established-decisions--do-not-rediscover"></a>
## Owning decisions

The [delivery guide](../implementation/README.md#owners) routes the unchanged design. The
[object owner](../../crates/search-source/search-revision-store/FUNCTIONS.md) owns plain storage;
[source rows](../../crates/search-source/search-source-registry/FUNCTIONS.md#current-state-and-conditional-application)
own guarded current-state updates. README files route to these rules instead of defining alternative algorithms.
Earlier platform, archive/import, Git-child and safe-display rules remain selected.

<a id="coverage-ledger"></a>
<a id="corrected-instructions"></a>
## Corrected entrypoints

Removed the object-store/history instructions that blocked plain integration and type relocation on #598.
Actual unpublished overlap remains protected; no encrypted implementation or second writer is introduced.

Removed the store README's competing Windows finalization and generic-GC recipes. Its links now lead to the
root platform owner and stopped-root collector. Removed abbreviated source-registry records/expected-head-only
signatures that omitted the non-reset state guard already required by FUNCTIONS.md. Historical source inventories
remain at immutable links; useful old section anchors are retained at their replacements.

<a id="delivery-remaining"></a>
## Remaining work

Implement the connected source/object path in [delivery order](../implementation/README.md#delivery-sequence).
#330 and #270 remain implementation work. This update does not implement storage, stale-work rejection, cleanup
or any earlier branch deletion. Use actual main callers when selecting a source cut.

<a id="evidence-status"></a>
<a id="execution-evidence"></a>
## Evidence status

| Check | Result / limit |
|---|---|
| Inputs | Six editable files matched their GitHub blob IDs at the pinned main revision. The older local snapshot was used only after those exact matches. |
| Rule comparison | Compared source family, object-store, root platform, source-row and maintenance instructions. The conflicts were in entrypoints, not a newly reproduced runtime defect. |
| Navigation / diff | Changed-page relative paths/fragments and preserved headings checked locally; diff whitespace checked. No full legacy/external-link crawl. |
| Publication | Compare the candidate and merged Git trees with the reviewed output; a prepared registry is not evidence of a successful merge. |
| Compiler/runtime | No Rust build, Clippy or tests run for this documentation-only cut. No source or dependency behavior changed. |
| Product | Foundation/current binaries, Windows durability, Qdrant integration, restart, archive/import, relevance and scale remain unverified. |

<a id="evidence-index"></a>
## Prior evidence

[R060 registry](https://github.com/UnknownAlienHuman/eliot-search/blob/4cb8dc4d61924ee0a1a2f3ce056b7086ac45c332/docs/audit/ANALYSIS_REGISTRY.md)
retains earlier checks and donor diagnostics. Keep one current checkpoint; change a rule at its owner and
update conflicting entrypoints/issues in the same cut, not another implementation specification.
