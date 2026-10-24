# Audit: resume and find evidence

Start at [Resume after interruption](ANALYSIS_REGISTRY.md#resume-after-interruption) in the analysis registry.
Work on the head of [PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610), branch
`codex/refactor-preparation-20261017`, not the old default-branch snapshot.

## Navigation

| Need | Read |
|---|---|
| Current stage and immediate next implementation | [Resume](ANALYSIS_REGISTRY.md#resume-after-interruption) |
| Decisions already established | [Established decisions](ANALYSIS_REGISTRY.md#established-decisions--do-not-rediscover) |
| Known product work | [Delivery groups](ANALYSIS_REGISTRY.md#delivery-remaining) |
| Area status and limits | [A001–A025 ledger](ANALYSIS_REGISTRY.md#coverage-ledger) |
| Full prior readsets and exact revisions | [Evidence index](ANALYSIS_REGISTRY.md#evidence-index) |
| What actually compiled or ran | [Execution evidence](ANALYSIS_REGISTRY.md#execution-evidence) |
| Baseline source counterexamples | [Distilled audit](ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md) |
| Intended package receiving owners | [Baseline disposition map](WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md) |
| Installed versus selected donors, official APIs and cost limits | [Donor register](DONOR_VERIFICATION_REGISTER_2026-10-09.md) |
| Native/shared effects and mixed-test dispositions | [Daemon caller map](../../bins/eliot-searchd/FUNCTIONS.md#inspected-callers-and-effects) and [test cutover](../implementation/TEST_CUTOVER.md#mixed-migration-tests-retained-for-behavior-extraction) |
| Disconnected query/rebuild/publication and actual CLI routes | [Composition boundaries](../../bins/eliot-searchd/FUNCTIONS.md#query-publication-and-rebuild-boundaries) and [CLI cutover](../../bins/eliot-search/FUNCTIONS.md#current-cli-selection-and-orphan-removal) |
| Adopted Markdown parser correction and pending execution | [Donor evidence](DONOR_VERIFICATION_REGISTER_2026-10-09.md#markdown-offset-iterator-released-fix-not-yet-adopted) and [materializer action](../../crates/search-prep/search-materializer/FUNCTIONS.md#markdown-donor-correction) |
| Typed settings semantics and retired config catalogues | [Layer/path recipe](../../crates/search-config/FUNCTIONS.md#sparse-layers-and-validation), [removal boundary](../../crates/search-config/FUNCTIONS.md#retired-repository-catalogues) and [donor constraints](DONOR_VERIFICATION_REGISTER_2026-10-09.md#typed-configuration-donor-boundaries) |

The registry is deliberately short. Detailed historical records remain at immutable checkpoint links in its
evidence index. Do not re-expand the current file with copied readsets, generate a replacement status database,
or interpret package removal as completed product behavior.

## Authority and preservation

[Architecture](../architecture/README.md) states purpose; [implementation](../implementation/README.md) and
nearest FUNCTIONS select the code change. Audit files record evidence and uncertainty, not permission, API
catalogues, schedules, runtime credentials or qualification gates.

Update the registry immediately with the actual inspected SHA/ranges, finding, decision and unrun work. Persist
it with the corresponding change on GitHub before reporting completion. Protect actual unpublished #598 overlap
and retained user data. No crawler, receipt system, evaluator authority or agent-control framework is needed.
