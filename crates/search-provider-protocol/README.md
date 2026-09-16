# search-provider-protocol

**C30 generic edge — Generic local provider protocol.**

**Status:** bounded protocol/session kernel implemented; complete public generic-edge
and standalone-grant round trips remain integration work.

Provides the generic local transport, binding and capability edge shared by CLI and optional client adapters.

## Owns

- canonical bounded frame and version negotiation semantics
- mutual authenticated hello, pairing and binding state
- sequence/replay/cancellation/flow control
- capability descriptor projection
- authenticated request/result/progress envelope lifecycle
- canonical bounded standalone-grant request body (requested ceilings only)

## Must not own

- Qdrant/redb/CAS access
- binding/access-policy persistence or grant authority
- client canonical writes or authority
- raw vendor plans/filters/point IDs
- compression or unbounded fragmentation in baseline

The standalone-grant body cannot choose binding, installation, principal,
operation or issued-grant identity. The daemon must derive those only after an
authenticated session and current authoritative policy snapshot. The live grant
command/response route remains unavailable until that complete chain is wired.

- **Delivery wave:** W1 / P01 transport; W8 / P14 integration
- **Soft source-line target:** 8,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
