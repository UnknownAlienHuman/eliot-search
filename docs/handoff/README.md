# Historical implementation handoff index

> **Superseded as implementation authority.** ADR 0005 and ADR 0006 define the current repository and
> product boundaries. Files in this directory may preserve package maps, past task decomposition and
> qualification ideas, but they cannot require `swarm/launch-state.toml`, assignment tickets, context
> artifacts, writer leases, acknowledgements, review receipts or wave advancement before Search work.

## Current implementation entry points

Use:

1. [Architecture entry point](../architecture/README.md) and Architecture Part I;
2. [ADR 0005](../adr/0005-standalone-search-product-and-controller-boundary.md);
3. [ADR 0006](../adr/0006-agent-analysis-framework-product-scope.md);
4. [agent-analysis product guide](../product/AGENT_ANALYSIS_FRAMEWORK.md);
5. nearest package `AGENTS.md` / `FUNCTIONS.md`;
6. current product issue/PR and compiled source reality.

A maintainer request, issue or PR plus one non-overlapping branch/worktree is sufficient to implement
Search. One writer per overlapping scope, exact bases and independent review remain useful engineering
practice without becoming a controller.

## Product-relevant historical packets

The following packets can be used as obligation checklists only. They do not authorize work and their
old dependency/wave ordering may be stale:

- `W1_IMPLEMENTATION_PACKET.md` — configuration, runtime owner, secrets, control and client shell;
- `W2_IMPLEMENTATION_PACKET.md` — source admission, identity, revisions, materialization and units;
- `W3_IMPLEMENTATION_PACKET.md` — lexical encoding, Qdrant, publication, pins and reclaim;
- `W4_IMPLEMENTATION_PACKET.md` — access, planning, retrieval, validation, results and continuations;
- `W5_IMPLEMENTATION_PACKET.md` — current workspace, observations, overlays and structure;
- `W6_IMPLEMENTATION_PACKET.md` — resolution, comparison and exact proof;
- `W7_IMPLEMENTATION_PACKET.md` — lifecycle, purge and restore;
- `W8_IMPLEMENTATION_PACKET.md` — standalone/client edge and optional leaf adapters;
- `W9_IMPLEMENTATION_PACKET.md` — product quality and Windows evaluation;
- `W10_IMPLEMENTATION_PACKET.md` — optional qualified depth profiles.

Always reconcile a packet against current code, Architecture Part I and accepted ADRs before using it.

## Controller-oriented historical material

`SWARM_*`, `TICKET_ISSUANCE_*`, `P00_DRAFT_CONTROL_PLANE.md`, old launch/read-set plans and related
controller schemas are not part of the Search product or its required development process. Their reusable
functionality belongs in `UnknownAlienHuman/eliot-swarm-controller` and later `eliot-memory-os`.

Residual active tooling and workflows are tracked for removal in issue #214. Do not extend, port or
“finish” those surfaces in Search.

## Qualification rule

Product qualification remains strict and separate:

- real Qdrant artifact/schema/retrieval/restart evidence;
- source → durable state → Qdrant → validated-result execution;
- Windows containment/secrets/named-pipe evidence;
- currentness, access, exact proof, recovery and installed-release evidence;
- large multi-repository scale/quality evidence (#215);
- document/research profile evidence for enabled formats (#216).

A historical packet, planning graph, structural validator or green controller workflow is not product
evidence.
