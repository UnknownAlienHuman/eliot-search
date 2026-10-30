# search-query-planner — remove the proof protocol

The existing package is a removal target, not a planner to complete before Search.
[FUNCTIONS.md](FUNCTIONS.md) owns direct dispatch and links the actual scope/evidence/window owners.

## Revision-specific evidence

The [previous source/caller inventory](https://github.com/UnknownAlienHuman/eliot-search/blob/69e85b04c0d1e3d4e8ef1742ec128ecd95cc8c04/crates/search-query/search-query-planner/README.md)
is historical. Inspect the actual current caller before replacing it; a Cargo dependency alone is not dispatch.

## Target

One typed request reaches one constructed handler. New Search/Exact captures current once; Continue/Expand use
the existing window. No one-leg plan, grant, capability registry or second copy of request/context is required.
Add only the handler for the current delivery stage; baseline Search does not wait for all optional operations.

## Remove after caller cutover

Move real selection into query composition, then remove old plan/leg/recipe types and exclusive fixtures.
A grouping helper is justified only for a selected multi-route operation, not a hypothetical future backend.
Do not revive a local lexical fallback or add a forwarding planner-v2.

## Completion

Actual dispatch reaches the receiving handler without the old planning protocol. Run
[minimal checks](../../../docs/implementation/WORKING_GUIDE.md#minimal-production-checks) for changed consumers.
Use the [review questions](../../../docs/implementation/WORKING_GUIDE.md#review-the-causal-path), not plan hashes.
