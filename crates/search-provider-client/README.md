# Reusable typed provider client

This crate owns the synchronous typed client moved from the CLI. The CLI resolves
independently trusted registration/credentials and opens the concrete platform
connection. The library consumes that connection/key with mandatory comparison
expectations and the original absolute setup deadline, then reuses the canonical
pairing, profile, grant, sequence, request/cancel and response state machines.

Default API exposes no TCP/token constructor or generic transport plugin.
`legacy-loopback-compatibility` preserves previously existing TCP signatures only;
the CLI enables it through its nondefault harness feature. Such pairing-only
sessions still cannot issue product grants without independently trusted identity.
#332 owns final native command/listener cutover and compatibility deletion.

Wire disposition is BYTE_COMPATIBLE. Seven private BLAKE3 operations retain the
accepted binding, pairing/profile and grant transcripts; generic shared prefixing
helpers are not equivalent. No persisted schema/profile migration is introduced.

`examples/typed_consumer.rs` is a separate Rust consumer crate target using public
session/grant/request/cancel APIs and borrowing actual coverage/handles unchanged.
It does not open an endpoint or pretend to authenticate a synthetic peer.

Source checks do not qualify native execution, installed operation or the full
product. Moved and new fixtures remain AUTHORED_NOT_RUN until integrated
qualification under #332/#215. The following cases must be checked at that gate:
wrong installation/incarnation/binding/grant; partial/oversized/truncated records;
profile and sequence/replay; progressive/terminal/cancel ordering; setup, grant
and recipe deadline phases; EOF and possible-write uncertainty; cross-session
handle/continuation refusal; content-free public diagnostics. Protocol/server
owned policy is not duplicated in this client.
