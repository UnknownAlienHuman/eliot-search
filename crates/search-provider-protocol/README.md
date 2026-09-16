# search-provider-protocol

**C30 generic edge — Generic local provider protocol.**

**Status:** bounded protocol/session kernel, canonical standalone-grant body,
grant-specific authenticated envelope and terminal lifecycle are implemented;
complete public generic-edge transport integration remains open.

Provides the generic local transport, binding and capability edge shared by CLI and optional client adapters.

## Owns

- canonical bounded frame and version negotiation semantics
- mutual authenticated hello, pairing and binding state
- sequence/replay/cancellation/flow control
- capability descriptor projection
- authenticated request/result/progress envelope lifecycle
- canonical bounded standalone-grant request body (requested ceilings only)
- dedicated standalone-grant proof domain and canonical envelope
- exact body-digest admission and one terminal in-flight release owner

## Must not own

- Qdrant/redb/CAS access
- binding/access-policy persistence or grant authority
- client canonical writes or authority
- raw vendor plans/filters/point IDs
- compression or unbounded fragmentation in baseline

The standalone-grant body cannot choose binding, installation, principal,
operation or issued-grant identity. Its dedicated envelope binds only protocol
version, server nonce, request identity and exact body digest. The daemon derives
server identities only after an authenticated session and current authoritative
policy snapshot.

The development loopback command shim remains separate and cannot expose grants
until it is replaced by the canonical `BoundSession` with a real durable binding
and policy source. Token-file, ACL or loopback state alone cannot fill that role.

- **Delivery wave:** W1 / P01 transport; W8 / P14 integration
- **Soft source-line target:** 8,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
