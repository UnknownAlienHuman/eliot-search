# Package assignment protocol

Every writer reads root/family/package instructions, exactly one assignment, relevant port-catalog
entries and accepted direct-dependency handoffs. W0 writers additionally read only the P00 contract-pack
files assigned in `docs/contracts/p00/README.md`.

## Boundaries

- one active writer, one Cargo package, one isolated worktree;
- writer edits only the assigned package path;
- root workspace, lockfile, toolchain, CI, architecture, contract pack, generated schemas, assignments,
  launch state and shared fixtures belong to the integration owner;
- missing/contradictory semantics use `CONTRACT_CHANGE_TEMPLATE.md`; do not invent a local type, port,
  adapter, reason code or fallback.

## Ownership rule

- shared serialized records: `search-contracts`;
- pure reusable meaning: `search-domain`;
- shared vendor-neutral traits: `search-ports`;
- capability mutable state/behavior: owning package;
- concrete adapters: their adapter package, constructed only by `eliot-searchd`.

## Operation contract

Every operation defines validated inputs, output identity, pre/postconditions, idempotency, deadline,
cancellation, bounded output and typed failures. Partial/degraded outcomes are data, not apparent
success. Package-local failures map explicitly before provider emission.

## Implementation order

1. turn owned invariants into failing tests;
2. consume accepted contract/port API digests;
3. define package-owned opaque state and public capability API;
4. add deterministic, negative, property, fault and security fixtures;
5. implement the smallest behavior that closes them;
6. complete the handoff with raw outputs, API/port digest and unresolved requests.

Compilation alone is not acceptance. Windows/Qdrant/redb/provider claims require executed environment
and artifact identity. No wildcard/floating git dependency or baseline Python/Node runtime.

Split review is mandatory before 8,500 total hand-written Rust lines; 10,000 including local tests is a
hard stop. Forwarding-only crates are forbidden.
