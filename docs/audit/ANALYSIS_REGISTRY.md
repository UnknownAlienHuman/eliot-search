# Analysis registry

Current checkpoint and evidence, not another specification. Algorithms live with their operation owners.

<a id="resume"></a>
<a id="resume-after-interruption"></a>
## Resume

**Checkpoint:** R059, 2026-10-29 — main/refactor status and archive/import safety instructions.
**Inspected main:** `623160a77572dac92ca9e19944295a0c9f44119c` (R058, merged PR #613).
**Scope:** nine Markdown files only; no Rust, Cargo/lockfile, configuration, workflow or test-source change.

Current documentation is on main. [PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610) remains
unmerged source work; its package removals and parser upgrade are not main capabilities. Preserve unpublished
#598 work. Next implementation remains [Foundation](../implementation/README.md#1-foundation--invocation-settings-and-root),
then one retained file through Qdrant to a verified CLI excerpt. Archive/import belongs to Stage 5, not that prerequisite set.

<a id="established-decisions--do-not-rediscover"></a>
## Current decisions

The [implementation guide](../implementation/README.md#owners) routes the existing owner rules and delivery order.
R055/R056 profile/context/window/whole-generation decisions are unchanged. The
[root owner](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md#archive-and-import-boundary) now defines
ordinary-open refusal for archives and incomplete imports; the
[maintenance recipe](../../crates/search-runtime/search-retention/FUNCTIONS.md#backup-is-a-completed-archive-not-a-runnable-clone)
owns archive layout, completion and explicit import. These are instructions to implement, not executed protection.

<a id="coverage-ledger"></a>
## Documentation changes

Corrected runtime/retention pages that described branch deletions as main state. Main still contains the old
retention package. Corrected the materializer and donor register: main pins/locks pulldown-cmark 0.13.1; 0.13.5
is on the unmerged branch. Shortened the runtime entrypoints instead of adding another status catalogue.

Closed two recipe gaps: a partial or marker-stripped archive must never look like a runnable root; a partially
imported destination must refuse ordinary open before backend recovery/cleanup, not only refuse queries.
Explicit import has one durable incomplete marker before imported data/effects; its removal follows verified
complete destination state. No phase journal, new service or Foundation dependency is introduced.

<a id="delivery-remaining"></a>
## Remaining work

Implement the connected path in [delivery order](../implementation/README.md#delivery-sequence). Port source work
from #610 only after tracing actual callers and running its minimum compiler/Clippy closure. Retire historical
master/packet/tooling inputs with their consumers. Do not restore an old validator or infer deletion from prose.

Archive/import still needs its actual root/control/object/publication implementation and later interruption cases.
The documentation correction does not complete #244, remove the legacy retention code or upgrade the parser.

<a id="evidence-status"></a>
<a id="execution-evidence"></a>
## Evidence status

| Check | Result / limit |
|---|---|
| Input | All nine edited input files reconstructed locally and matched their GitHub blob IDs at the pinned main revision. |
| Source facts | Root Cargo manifest and Markdown lock entry read at that same revision: retention is a member; pulldown-cmark is 0.13.1. |
| Review | Compared root, object, source-head, preparation, control and publication recipes; archive/import changes are static failure-path analysis, not an incident reproduction. |
| Navigation / diff | Outgoing relative links and fragments in the changed pages and diff whitespace checked locally; existing external URLs and the full legacy graph were not crawled. |
| Scope | Markdown only. No product source, dependency, permanent checker or workflow added. |
| Compiler/runtime | No Rust build, Clippy or tests run. Main and historical branch compiler evidence remain separate. |
| Product | Foundation/current binaries, Windows durability, selected Qdrant integration, restart, archive/import, relevance and scale remain unverified. |

<a id="evidence-index"></a>
## Prior evidence

[R058 registry](https://github.com/UnknownAlienHuman/eliot-search/blob/623160a77572dac92ca9e19944295a0c9f44119c/docs/audit/ANALYSIS_REGISTRY.md)
retains previous checks and earlier readsets. The [donor register](DONOR_VERIFICATION_REGISTER_2026-10-09.md)
separates selected mechanisms from the installed revision and executed behavior.

Keep one current checkpoint. Update a rule at its owner and conflicting references in the same change; do not
accumulate a second specification or audit diary.
