# Analysis registry

Current documentation, source baseline and executed evidence. Algorithms live with their owners.

<a id="resume"></a>
<a id="resume-after-interruption"></a>
## Resume

**Checkpoint:** R057 — publish the current documentation to `main` independently of the source refactor.
**Documentation input:** [R056, ce441f78](https://github.com/UnknownAlienHuman/eliot-search/commit/ce441f7890c47739e3424314a9e03cd384b502b4).
**Main source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`; this landing changes Markdown only.
**Unmerged source work:** [PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610), branch
`codex/refactor-preparation-20261017`, inspected head `ce441f7890c47739e3424314a9e03cd384b502b4`.

Read the current [delivery stage](../implementation/README.md#delivery-sequence), its owner recipes and actual
callers. Foundation remains first, then one retained file through real Qdrant to a verified CLI excerpt.
Do not restart a broad audit or wait for optional systems. Preserve unpublished #598 work.

<a id="established-decisions--do-not-rediscover"></a>
## Current decisions

The [architecture](../architecture/README.md) and [implementation owner map](../implementation/README.md#owners)
remain selected. R056's owner corrections are carried forward, not re-opened by this publication.
Historical source findings and completed deletions describe their named commits or the refactor branch; they
must not be interpreted as code already merged into `main`.

<a id="coverage-ledger"></a>
## Documentation work

The R056 Markdown set is published with corrected main/source status, quickstart and tooling/evidence notes.
`search-eval` remains in the main source baseline; its README is kept because `src/lib.rs` includes it.
Legacy documents/configuration/tooling are retained when removing them would require a source cut. The current
reading path in the [index](../README.md) supersedes their old architecture, wave, ticket and acceptance rules.

Documentation-only changes can land after their own checks. A coherent source cut can land after minimal
compiler/Clippy for its actual consumers; neither waits for the entire programme to finish. Do not use the large
unqualified source PR as the vehicle for publishing unrelated documentation.

<a id="delivery-remaining"></a>
## Remaining product work

[Delivery sequence](../implementation/README.md#delivery-sequence): Foundation → one-file Search → restart,
inventory and complete generations → measured conditional incrementality → Exact/session → selected depth.
No product stage is completed by this documentation landing. Before landing source from #610, isolate a coherent
change, inspect actual callers, run its smallest locked check/Clippy closure and record the result at that head.

<a id="evidence-status"></a>
<a id="execution-evidence"></a>
## Evidence status

| Check | Evidence / limit |
|---|---|
| Exact documentation input | Reconstructed R056 tree equals `3a0e615a7a1c83fe166802eac6bc3a7a3c6b51bb`: R055 archive plus all 36 modified Markdown blobs and two deletions. |
| Prior navigation check | R056 recorded 200 Markdown files, 599 relative links, zero missing paths/anchors. This is evidence for that exact snapshot, not a new crawl of legacy documents retained on main. |
| Landing boundary | Based on main `c8e38b6f`; only Markdown is selected. Source, Cargo manifests/lockfile, configuration and workflows are not imported from #610. |
| Publication check | Final PR comparison and merge metadata identify the delivered tree and merge outcome; do not infer a successful merge from this prepared file alone. |
| Compiler/runtime | No build, Clippy or runtime test is claimed by this docs-only landing. Earlier R031 compiler results are narrow historical evidence, not a current whole-product build. |
| Product | Foundation/current binaries, Windows finalization/durability, selected Qdrant integration, restart, end-to-end Search, relevance and scale remain unverified. |

<a id="evidence-index"></a>
## Prior evidence and maintenance

[R056 registry](https://github.com/UnknownAlienHuman/eliot-search/blob/ce441f7890c47739e3424314a9e03cd384b502b4/docs/audit/ANALYSIS_REGISTRY.md)
links earlier checkpoints and executed checks. [Donor evidence](DONOR_VERIFICATION_REGISTER_2026-10-09.md)
remains separate from installed dependencies and product behavior.

Keep one current checkpoint, exact source revisions, actual checks and next connected step. Change each rule at
its owner and update conflicting references. Do not accumulate a second specification or a job history here.
