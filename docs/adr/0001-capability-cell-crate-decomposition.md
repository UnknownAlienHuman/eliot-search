# ADR 0001 — Capability boundaries and Cargo packaging

- **Status:** accepted
- **Date:** 2026-08-28; packaging clarification 2026-10-17
- **Scope:** implementation packaging only
- **Architecture:** ELIOT Search 8.4, especially S5, S31 and H1

## Context

Architecture capability families describe ownership. A Cargo package is useful when it isolates a real
state/effect, security, dependency, replacement, failure, test or lifecycle boundary. S31 calls its crate
list recommended focused crates; it does not require one crate per capability cell, type or agent.

The original one-agent/one-crate and 7,500/10,000-line targets were review heuristics. Applied as quotas,
they encourage forwarding packages and file splitting without removing duplicated behavior. Current
execution uses one source writer and narrow read/research/review subagents.

## Decision

1. Preserve C00–C30 ownership and every Architecture 8.4 product invariant.
2. Treat `search-runtime`, `search-source`, `search-prep`, `search-index-qdrant` and `search-query` as
   organizational families. Package boundaries follow demonstrated independence, not the cell count.
3. Keep `search-domain` a small pure invariant kernel. Keep optional provider/ELIOT/Research leaves
   separate where their dependencies and lifecycle differ. Public ports remain vendor-neutral;
   composition belongs to the actual daemon owner, never forwarding crates.
4. Remove obsolete implementations and duplicate state/codecs before splitting or merging packages.
   Consolidation is valid when the remaining code has one actual owner and no independent boundary;
   inventory contracts, consumers, features and persisted bytes first. No arbitrary crate-count target.
5. Review growing modules/packages for mixed responsibilities. The former 7,500/10,000-line targets
   are signals, not gates against a bounded shrinking slice and not a requirement to create more crates.
6. Store relevant package guidance once and link its normative source. Do not copy the architecture
   or execution protocol into every agent packet.

## Consequences

Smaller owned mechanisms and explicit consumers matter more than file count. Cross-owner cutovers use
one bounded manager slice under [EXECUTION_PROTOCOL](../product/EXECUTION_PROTOCOL.md). A package move
alone does not count as code reduction or completed integration.

This clarification changes packaging guidance only. Capability semantics, topology obligations,
protocols and the embedded Architecture Part I body/hash are unchanged.
[Original accepted packaging record](https://github.com/UnknownAlienHuman/eliot-search/blob/927265d1dd71f3d355131f38ac73365e6558a77e/docs/adr/0001-capability-cell-crate-decomposition.md).

## Rejected alternatives

- Crate-per-type/agent, forwarding facades or new public wrappers without an independent owner.
- One undifferentiated package that erases genuine security, dependency or replacement boundaries.
- File shuffling, minification or duplicate per-agent prose presented as simplification.
