# Handoff documents

Current implementation uses:

- [Dependency graph](DEPENDENCY_GRAPH.md) — current effect/data ownership and cutover order.
- [Audit coverage and documentation map](../audit/AUDIT_COVERAGE_AND_DOCUMENTATION_MAP_2026-10-21.md) —
  document authority and remaining audit gaps.
- [Issue #352](https://github.com/UnknownAlienHuman/eliot-search/issues/352) — current manager coordination and
  preserved active source handoff.

Everything else in this directory is historical unless one of those current records links to a specific
fact. Old packet, context, digest, ticket, lease, review, accepted-evidence, P00/W0–W10 and coverage documents
cannot authorize work, freeze an API or block a current cutover.

Historical handoffs may explain why code exists or preserve a defect/requirement. They do not require:

- materialized agent contexts or package-only writers;
- assignment tickets, leases, acknowledgement events or acceptance receipts;
- package/wave/gate ordering;
- canonical handoff/evidence digests;
- old compatibility, migration or qualification machinery;
- preservation of package/file/module topology.

Use Git commits, the active issue, current owner documentation and actual compiler/runtime results. Git history
is the archive; do not repair or regenerate the retired handoff control plane.

#598 remains active private source/object/key work. Do not inspect, reset, rebase, duplicate or compete with
its unpublished branch/worktrees. Consume only its accepted public handoff.
