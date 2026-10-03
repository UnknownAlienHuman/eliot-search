# search-ports

**C00 support — vendor-neutral capability and infrastructure ports.**

**Status:** substantive trait and conformance source exists for control, source, preparation, lexical,
index, access, exact, handle, clock and process boundaries. This package deliberately provides no concrete
adapter or mutable runtime implementation.

It depends only on Eliot-owned contracts and keeps Qdrant, redb, filesystem, Windows and client-vendor
types out of shared public APIs.

## Owns

- vendor-neutral port traits and operation contexts;
- cancellation, deadline and finite-result semantics at port boundaries;
- idempotency and possible-external-write outcome contracts;
- fake/scripted conformance helpers for consumers;
- the public boundary that prevents vendor/OS/database types from leaking upward.

## Must not own

- concrete redb, Qdrant, filesystem, process, secret-store or client implementations;
- mutable runtime state or capability algorithms;
- duplicate wire/domain records already owned by `search-contracts`;
- policy interpretation owned by `search-domain` or capability packages;
- a generic runtime service locator or floating adapter selection.

The package is `SOURCE`; that is its intended role. Concrete product readiness is established by each
adapter and daemon composition, not by adding behavior here.

- **Product area:** Architecture S4/H4 shared boundaries
- **Agent instructions:** [AGENTS.md](AGENTS.md)
- **Current status matrix:** [../../docs/product/IMPLEMENTATION_STATUS.md](../../docs/product/IMPLEMENTATION_STATUS.md)
