# Current handoff and ownership navigation

This directory contains the small current ownership graph. It is not an agent-control plane.

- [Dependency graph](DEPENDENCY_GRAPH.md) — current data/effect owners and cutover relationships.
- [Implementation delivery sequence](../implementation/README.md#delivery-sequence) — shortest path to a working application.
- [Analysis registry](../audit/ANALYSIS_REGISTRY.md#resume-after-interruption) — current checkpoint, evidence and unrun work.
- [Issue #352](https://github.com/UnknownAlienHuman/eliot-search/issues/352) — overlap coordination and current programme summary.

## Taking a work slice

1. Verify the actual PR head and read the registry resume point.
2. Select one delivery slice and load only its owner recipe plus real producers/consumers.
3. Preserve useful behavior, connect the real path and remove displaced mechanisms/fixtures together.
4. Record inspected paths, decision, changed owner and unrun work in the existing registry.
5. Update canonical docs, the owning issue, #352 and PR summary when their current truth changes.

Use ordinary commits, PR review and actual compiler/runtime results. A packet, lease, receipt, package row or
agent assignment cannot freeze APIs or establish completion.

The former packet/context/digest/ticket/lease/review/P00/W0–W10/swarm documents were removed. Git history is
their archive; do not regenerate them or build a replacement status/approval database.

#598 remains protected unpublished object/key work. Do not inspect, reset, rebase, duplicate or compete with
its private branch/worktrees. Consume only its accepted public handoff; its retired six-domain/purge/
compatibility programme is not repository-wide authority.
