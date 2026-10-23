# Audit: resume and find evidence

Start at [Resume after interruption](ANALYSIS_REGISTRY.md#resume-after-interruption) in the analysis registry.
Work on the head of [PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610), branch
`codex/refactor-preparation-20261017`, not the old default-branch snapshot.

## Navigation

| Need | Read |
|---|---|
| Current stage and immediate next question | [Resume](ANALYSIS_REGISTRY.md#resume-after-interruption) |
| Answered questions versus remaining static scope | [S1–S5 worksets](ANALYSIS_REGISTRY.md#remaining-static-audit) |
| Known code work versus unfinished research | [Delivery groups](ANALYSIS_REGISTRY.md#delivery-remaining-separate-from-static-audit) |
| Area status and limits | [A001–A025 ledger](ANALYSIS_REGISTRY.md#coverage-ledger) |
| Previous findings by topic, source paths and exact revision | [Pass/topic index](ANALYSIS_REGISTRY.md#evidence-index-by-pass-and-topic) |
| What actually compiled or ran | [Execution evidence](ANALYSIS_REGISTRY.md#execution-evidence) |
| Baseline source counterexamples | [Distilled audit](ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md) |
| Intended package receiving owners | [Baseline disposition map](WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md) |
| Installed versus selected donors, official APIs and cost limits | [Donor register](DONOR_VERIFICATION_REGISTER_2026-10-09.md); earlier evidence is indexed by pass |
| Native/shared effects and mixed-test dispositions | [Daemon caller map](../../bins/eliot-searchd/FUNCTIONS.md#inspected-callers-and-effects) and [test cutover](../implementation/TEST_CUTOVER.md#mixed-migration-tests-retained-for-behavior-extraction) |
| Disconnected query/rebuild/publication and actual CLI routes | [Composition boundaries](../../bins/eliot-searchd/FUNCTIONS.md#query-publication-and-rebuild-boundaries) and [CLI cutover](../../bins/eliot-search/FUNCTIONS.md#current-cli-selection-and-orphan-removal) |
| Released Markdown parser correction and local adoption gap | [Donor evidence](DONOR_VERIFICATION_REGISTER_2026-10-09.md#markdown-offset-iterator-released-fix-not-yet-adopted) and [materializer action](../../crates/search-prep/search-materializer/FUNCTIONS.md#markdown-donor-correction) |
| Typed settings semantics and retired config catalogues | [Layer/path recipe](../../crates/search-config/FUNCTIONS.md#sparse-layers-and-validation), [removal boundary](../../crates/search-config/FUNCTIONS.md#retired-repository-catalogues) and [donor constraints](DONOR_VERIFICATION_REGISTER_2026-10-09.md#typed-configuration-donor-boundaries) |

The disposition map's 52 rows describe the original baseline; do not recreate removed packages. A donor's API,
a constructor, a successful fixture or a heading saying "current" is not proof of connected behavior.

## Authority and preservation

[Architecture](../architecture/README.md) states purpose; [implementation](../implementation/README.md) and
nearest FUNCTIONS select the code change. Audit files record evidence and uncertainty, not permission, API
catalogues, schedules or qualification gates. The registry is the single continuation record, not this index.

The [former closure map](AUDIT_COVERAGE_AND_DOCUMENTATION_MAP_2026-10-21.md) is superseded as a status source.
Its archived text and the earlier detailed checkpoints remain linked. Do not describe incomplete static review
as "only runtime work remains", or erase inspected-path provenance when shortening a report.

Update the existing registry with the actual inspected SHA/ranges, finding, changed owner and unrun work. Persist
it with the change on GitHub before reporting completion. Protect actual unpublished #598 overlap and retained
user data. No new generated status database, crawler, receipt system or agent-control framework is needed.
