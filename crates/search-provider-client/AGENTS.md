# Agent contract — search-provider-client

Own one synchronous typed client session/correlation engine. Consume a concrete
platform connection and non-Clone BindingKey with mandatory caller-owned trusted
comparison coordinates. Pairing/profile/grant policy remain the protocol/server
owners; expectations are not an authenticated receipt or authority.

Keep LocalByteStream, framing I/O, setup budget and mutable state private. One
reader, one key, one sequence/correlation ledger, one cancellation slot; no stream
clone, background task, reconnect, pooling, discovery, fallback or blind replay.
Preserve the original absolute setup deadline and one deadline per operation.
Raw I/O errors and credential/path/query/handle/proof material never enter public
Debug, Display or source(). Reuse accepted protocol types and exact wire bytes.

Dependencies are search-contracts, search-ports, search-provider-protocol, the
concrete search-provider-transport-windows owner and the accepted exact BLAKE3
pin. No database, Qdrant, async runtime or external networking framework.

Nondefault legacy-loopback-compatibility retains old TCP APIs only until #332
atomically cuts over/deletes their consumers. A missing independently trusted
installation cannot issue a product grant, even with this feature enabled.

Compile the library and direct CLI/example consumers with Rust 1.98 locked
checks and strict production Clippy. Author causal fixtures with changes;
execution/native/fault/installed qualification follows the execution protocol.
Compilation alone is not product qualification.
