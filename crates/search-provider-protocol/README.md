# search-provider-protocol

**C30 generic edge — Generic local provider protocol.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Implement the generic local transport, binding and capability edge shared by CLI and optional client adapters.

## Owns

- named-pipe framing and version negotiation
- mutual authenticated hello and binding state
- sequence/replay/cancellation/flow control
- capability descriptor projection
- request/result/progress envelope lifecycle

## Must not own

- Qdrant/redb access
- client canonical writes or authority
- raw vendor plans/filters/point IDs
- compression or unbounded fragmentation in baseline

- **Delivery wave:** W1 / P01 transport; W8 / P14 integration
- **Soft source-line target:** 8,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
