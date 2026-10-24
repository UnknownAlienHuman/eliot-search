# Audit and evidence navigation

Audit files explain what was inspected, what was concluded and what remains unrun. They do not authorize code,
freeze APIs or create readiness credentials. Start at the
[registry resume point](ANALYSIS_REGISTRY.md#resume-after-interruption) on the head of
[PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610).

## Current records

| Need | Read |
|---|---|
| Current checkpoint and next slice | [Analysis registry](ANALYSIS_REGISTRY.md#resume-after-interruption) |
| Decisions already established | [Established decisions](ANALYSIS_REGISTRY.md#established-decisions--do-not-rediscover) |
| Delivery groups and area status | [Delivery remaining](ANALYSIS_REGISTRY.md#delivery-remaining), [coverage ledger](ANALYSIS_REGISTRY.md#coverage-ledger) |
| What actually compiled or ran | [Execution evidence](ANALYSIS_REGISTRY.md#execution-evidence) |
| Full earlier readsets/revisions | [Evidence index](ANALYSIS_REGISTRY.md#evidence-index) |
| Installed/selected donors and upstream limits | [Donor register](DONOR_VERIFICATION_REGISTER_2026-10-09.md) |
| Baseline package dispositions | [Workspace member disposition](WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md) |
| Baseline counterexamples and root causes | [Distilled audit](ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md) |
| Legacy coverage-map links | [Superseded coverage pointer](AUDIT_COVERAGE_AND_DOCUMENTATION_MAP_2026-10-21.md) |

R032 reconciles canonical documentation and fixes stale R031/#441/Qdrant/test-status claims. Detailed historical
records remain at immutable commit links in the registry; do not copy them back into the live file or generate a
replacement status database.

## Authority

- [Architecture](../architecture/README.md) owns product purpose and observable behavior.
- [Implementation](../implementation/README.md) and nearest `FUNCTIONS.md` own mechanisms and source seams.
- Audit records own evidence, uncertainty and historical rationale only.

Update the registry with the inspected SHA/paths, finding, decision, changed owner and unrun work in the same
published change. Protect #598 private overlap and retained user data. No crawler, receipt service, evaluator
authority or agent-control framework is needed.
