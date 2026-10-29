# Analysis registry

Current checkpoint and evidence. Algorithms remain with their operation owners.

<a id="resume"></a>
<a id="resume-after-interruption"></a>
## Resume

**Checkpoint:** R060, 2026-10-29 — controlled Git subprocesses and safe result/diagnostic presentation.
**Inspected main:** `46258450adafbba5535dc747d59f5b46ceef3cbc` (R059, merged PR #614).
**Scope:** five Markdown files. No product source, Cargo/lockfile, configuration, workflows or tests changed.

Documentation is on main; [PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610) remains separate
source work. Its removals/upgrades are not installed main behavior. Preserve unpublished #598 work.
Next remains [Foundation](../implementation/README.md#1-foundation--invocation-settings-and-root), then one
retained file through Qdrant to a verified CLI excerpt. Optional Git ingestion is not a new baseline prerequisite.

<a id="established-decisions--do-not-rediscover"></a>
## Owning decisions

The [delivery guide](../implementation/README.md#owners) retains the current architecture and owner map.
[Git acquisition](../../crates/search-source/search-safe-reader/FUNCTIONS.md#git-child-boundary) now requires
explicit fsmonitor suppression and controlled executable/arguments/environment/metadata roots.
[CLI display](../../bins/eliot-search/FUNCTIONS.md#untrusted-display-text) owns terminal-safe rendering and
post-escaping bounds. Client diagnostics and structured projection link to that owner instead of copying it.
R059's archive/import rules and earlier profile/context/window decisions are unchanged.

<a id="coverage-ledger"></a>
## Corrected instructions

Direct argv plus no-replace/no-lazy-fetch/no-optional-locks did not stop configured fsmonitor execution during
index enumeration. The recipe now supplies the missing command boundary, rejects weakened retries and retains
execution-only acquisition context. It does not add another process platform.

Proper JSON serialization alone did not define human terminal safety. Excerpts, filenames and child diagnostics
now have one explicit display rule without mutating retained evidence or coordinates. Capture-limit exhaustion
must keep draining stderr or terminate/reap the child, not stop reading a live pipe and deadlock output.
Issues #335 and #332 should use main's owning recipes, not the older refactor-branch copies.

<a id="delivery-remaining"></a>
## Remaining work

Implement the actual connected consumers in [delivery order](../implementation/README.md#delivery-sequence),
then run minimal locked compiler/Clippy for the changed owners and binaries. Git #335 and CLI #332 remain open.
No Git adapter, terminal renderer or diagnostic drain fix is delivered by this documentation change.

<a id="evidence-status"></a>
<a id="execution-evidence"></a>
## Evidence status

| Check | Result / limit |
|---|---|
| Input | All five edited files match their GitHub blob IDs at the pinned main revision; local reference material was not assumed to be current. |
| Git donor source | Inspected Git 2.51.0 revision c44beea4: ls-files reads the index, post-read processing invokes fsmonitor, and the configured hook uses a shell. Exact source links are in the acquisition recipe. |
| Executed diagnostic | Disposable Linux repository, installed Git 2.47.3, harmless marker-writing hook: old command exited 0 with one hook invocation; core.fsmonitor=false exited 0 with zero invocations and identical index entries. Not an Eliot test or a Git 2.51/Windows qualification. |
| Output boundary | Read main public_client.rs::drain_until_terminal and official terminal-control documentation. No source-to-terminal exploit or new renderer was executed. |
| Navigation / diff | Newly added relative links and named anchors checked against the changed files and inspected implementation guide; diff whitespace checked. No whole-repository or external-link crawl. |
| Compiler/runtime | No Rust build, Clippy or product tests run. The donor diagnostic is the only new execution evidence. |
| Product | Foundation/current binaries, Windows durability, selected Qdrant integration, restart, archive/import, relevance and scale remain unverified. |

<a id="evidence-index"></a>
## Prior evidence

[R059 registry](https://github.com/UnknownAlienHuman/eliot-search/blob/46258450adafbba5535dc747d59f5b46ceef3cbc/docs/audit/ANALYSIS_REGISTRY.md)
preserves prior checks. The [donor register](DONOR_VERIFICATION_REGISTER_2026-10-09.md) distinguishes selected,
installed and executed behavior. Update rules at their owner; do not accumulate a second specification here.
