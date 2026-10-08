# search-ports

**C00 support — vendor-neutral capability and infrastructure ports.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

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
