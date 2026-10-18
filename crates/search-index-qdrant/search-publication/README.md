# search-publication

Coordinate replacement or retirement of one prepared source membership so queries
see a coherent published view. The useful work is the exact change set, publication
of the new view, and recovery of external writes whose result is unresolved.

The current [coordinator](src/machine.rs) is process-local bookkeeping. Its methods
accept observations and receipts; they do not write redb, call Qdrant, or publish
the daemon's query snapshot. Coordinator construction and restoration currently
appear in tests; the daemon has a separate
[Publisher](../../../bins/eliot-searchd/src/publication_composition/kernel/publisher.rs).
Unify the real producer and consumer around one publication owner.

## Behavior to keep

- Prepare from the complete membership and its actual revision/profile. Reuse
  unchanged points; give changed immutable point content a new physical ID.
- Finish the required index changes before committing the visible route/epoch.
  Keep a query's old view readable until its last guard is released.
- Retire a membership explicitly. Source removal must not fabricate a valid empty
  representation to satisfy a preparation API.
- Persist enough information to resolve a write that may have reached Qdrant.
  Ordinary local preparation and completed-operation history need no second ledger.

[FUNCTIONS.md](FUNCTIONS.md) distinguishes actual APIs from the integration work.
The current phases, proof-shaped records and digest layout are implementation
material to simplify with callers, not a new public contract to reproduce.

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md), and [agent notes](AGENTS.md).
