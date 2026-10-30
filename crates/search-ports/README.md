# search-ports — delete the global abstraction catalogue

The current product uses concrete owners, not a global catalogue of ports, contexts, receipts and fake effects.
[FUNCTIONS.md](FUNCTIONS.md) routes the affected legacy family to its replacement owner.

## Established audit result

[Historical counts and caller inventory](https://github.com/UnknownAlienHuman/eliot-search/blob/69e85b04c0d1e3d4e8ef1742ec128ecd95cc8c04/crates/search-ports/README.md)
explain this removal target; they are not a current caller census or a required package topology.

## Replacement rule

Start with a concrete call. A small consumer-local interface is appropriate only for a real variation or
dependency boundary; [implementation choices](../../docs/implementation/WORKING_GUIDE.md#implementation-choices)
define the criteria. No ports-v2, service locator or effect bus.

## Context and effect semantics

The [owning recipe](FUNCTIONS.md#execution-versus-query-state) separates execution from query state and routes
uncertain effects. A normal local call creates neither a durable receipt nor another copy of the same request.

## Deletion

Follow the current delivery stage and actual dependencies, not a package-by-package rewrite queue. Cut over one
real consumer path, then remove its unused traits/adapters/fixtures. Delete the crate after its final caller.

## Verification

Use [actual-consumer checks](../../docs/implementation/WORKING_GUIDE.md#minimal-production-checks).
Port counts, fake matrices and generated catalogue completeness do not prove the connected result.
