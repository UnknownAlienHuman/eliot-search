# Generic client and provider-edge contracts

This directory contains bounded contracts for the standalone local Search client and optional leaf
adapters such as ELIOT, generic agent tools and Research exports.

- [`W8_GENERIC_CLIENT_EDGE_CONTRACTS_1.0.md`](W8_GENERIC_CLIENT_EDGE_CONTRACTS_1.0.md) — transport,
  binding, grant, capability, request/result, handle expansion and authority boundaries.
- [`manifest.toml`](manifest.toml) — historical machine owner/read-set inventory.
- [`../config/W8_CLIENT_EDGE_SETTINGS_1.0.md`](../config/W8_CLIENT_EDGE_SETTINGS_1.0.md) — client-edge
  product settings.
- [`../../qualification/client-edge/README.md`](../../qualification/client-edge/README.md) — future
  executed product evidence; all probes remain unavailable until run.

## Product boundary

The standalone CLI and every optional adapter use the same daemon, source state, Qdrant route, access
compiler, query planner, candidate validator and result projector. No client opens redb or Qdrant
directly, and no adapter creates a second query or authority path.

The ELIOT adapter receives no task, admission or finish authority. A generic MCP/JSON/stdio adapter also
remains a leaf over this protocol; it does not become an agent controller. The generic adapter is tracked
by #219.

## Agent-analysis surface

The exact eleven P00/v1 recipes remain the current closed wire set. They cover exact navigation, entity
inspection/exploration, comparison, corpus profile/delta, provenance, exact scan and handle expansion.
They do **not** yet provide a first-class general free-text retrieval request or scope-orientation result.
That additive versioned contract is tracked by #213; clients must not emulate it through raw Qdrant
queries or silently overload `find_text@1`.

The supported management surface must also let ordinary users register sources, create corpora and select
immutable repository portfolios without editing internal IDs. That work is tracked by #218. The concrete
lexical/structural ranking profile behind general agent retrieval is tracked by #221.

Agent-facing results should be compact, source-backed and explicit about ambiguity, freshness and
coverage. Recommended reading and suggested exact checks are navigation hints, not task decisions or
proof until their source handles/scan plans are executed.

## Implementation status

See [`../product/IMPLEMENTATION_STATUS.md`](../product/IMPLEMENTATION_STATUS.md). The provider/session
kernel and substantial CLI source exist, but complete named-pipe composition, real indexed recipe
execution and agent-tool integration are not qualified or enabled.

## Implementation authority

Architecture Part I, accepted ADRs and accepted product contracts are authoritative. `swarm/launch-state.toml`,
ticket drafts, leases and historical W8 stage metadata do not authorize or block implementation. ADR 0005
and ADR 0006 supersede lower-precedence statements to the contrary.

Optional ELIOT, generic-agent and Research profiles remain disabled by default and are not required for
standalone startup. Their unavailability must not remove the ordinary local Search client.
