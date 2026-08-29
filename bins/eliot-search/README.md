# eliot-search

**Status:** binary package boundary and agent contract only; runtime behavior is intentionally unimplemented.

Expose standalone commands strictly through the generic provider protocol.

## Owns

- argument parsing
- local binding/bootstrap UX
- request construction
- bounded result rendering
- doctor command transport

## Must not own

- opening redb, CAS or Qdrant
- reimplementing query/access logic
- minting unbounded grants
- rendering hidden raw payloads

- **Delivery:** W1 shell, commands added by owning packages
- **Soft source-line target:** 4,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
