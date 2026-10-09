# search-ports

**C00 support — vendor-neutral capability and infrastructure ports.**

**Status:** `SOURCE_PRESENT`; shared port interfaces and conformance support exist, while downstream product integration and qualification remain owner-specific. See [central package status](../../docs/product/PACKAGE_STATUS.toml). Current consumers: #235/#116 and the #97 graph.

This package owns the shared trait boundary between pure contracts/domain logic, capability
orchestration and concrete adapters. It depends only on `search-contracts`.

## Owns

- vendor-neutral port traits and operation contexts
- idempotency, cancellation, deadline and bounded-result semantics at port boundaries
- fake/in-memory conformance interfaces for consumer tests
- proof that vendor, OS and database types cannot cross public APIs

## Must not own

- concrete redb, Qdrant, filesystem, process, secret-store or client implementations
- mutable runtime state or capability algorithms
- duplicate wire/domain records already owned by `search-contracts`
- policy interpretation owned by `search-domain` or a capability package

- **Delivery wave:** W0 / P00 after accepted `search-contracts`
- **Soft source-line target:** 5,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
