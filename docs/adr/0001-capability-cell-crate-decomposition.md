# ADR 0001 — Capability-cell crate decomposition for agent-swarm implementation

- **Status:** accepted
- **Date:** 2026-08-28
- **Scope:** implementation packaging only
- **Architecture:** ELIOT Search 8.4, especially S5, S31 and H1

## Context

The initial repository grouped C03–C07 into `search-source`, C13–C17 into
`search-index-qdrant`, and C18–C27 into `search-query`. Those groupings are useful architecture
families, but they are too broad for the required execution model:

- one writer agent owns one crate;
- an agent should not repeatedly load the 145 KB master document;
- each package should target fewer than 10,000 hand-written Rust lines;
- independent security, failure and replacement boundaries should be reviewable in isolation.

Architecture S31 calls its crate list “recommended focused crates” and states that a capability cell
becomes a separate crate when it has a real dependency, replacement, test or context boundary.
Every cell selected here has at least one such boundary.

## Decision

1. Preserve C00–C30 ownership and every Architecture 8.4 invariant unchanged.
2. Treat `search-runtime`, `search-source`, `search-prep`, `search-index-qdrant` and `search-query`
   as organizational capability families, not Cargo packages.
3. Create one Cargo package for each capability cell with a real independent boundary.
4. Keep `search-domain` as a small shared pure invariant kernel; it owns no external capability.
5. Split C30 into a generic provider-protocol package and optional ELIOT/Research leaf adapters
   because their dependencies and replacement lifecycles differ.
6. Keep public ports vendor-neutral. Composition happens in `eliot-searchd`, not in forwarding crates.
7. Target `src/` below 7,500 lines and require a split review before 10,000 total hand-written Rust
   lines in a package.

## Consequences

- Swarm tasks have bounded context and non-overlapping write ownership.
- `search-query` and `search-source` no longer become mega-crates.
- Cargo has more packages, but compilation, testing and review can be parallelized by capability.
- Cross-capability contract changes become explicit requests instead of opportunistic edits.
- The Architecture 8.4 embedded body and SHA-256 remain unchanged because capability ownership,
  topology, protocols and invariants are not modified.

## Rejected alternatives

- **Keep ten broad crates:** conflicts with one-agent/one-crate and likely exceeds the line target.
- **Create forwarding facade crates:** explicitly forbidden; they add no causal owner or test seam.
- **Create crate-per-type:** rejected; packages are capability boundaries, not namespace wrappers.
- **Duplicate architecture prose per agent:** rejected; per-package instructions contain only the
  relevant normative slice.
