# Working on search-provider-protocol

Read the [architecture](../../docs/architecture/README.md),
[implementation](../../docs/implementation/README.md) and [FUNCTIONS.md](FUNCTIONS.md).

Implement one bounded length-prefixed typed JSON exchange used by the real CLI and daemon. Keep one active
data request, a serviced control path, bounded reads/writes and complete terminal decoding. A healthy handled
request has one terminal; a failed or disconnected transport can deliver none. Do not invent an acknowledgement
journal to promise otherwise.

FUNCTIONS owns framing, backpressure, clock handoff and terminal visibility. A synchronous caller API must not
block cancellation behind write-all/read-all. A partial frame is finished within its bound or the session closes;
Cancel is not inserted into its byte stream. Data capacity and completed-request history cannot gate close/cancel.

Use process/OS ownership for trusted child stdio. Remove recipes, manual JSON/base64 grammar, pairing/MACs,
grants/capability digests, replay history, binding lifecycle, source-handle cleanup records and legacy line/TCP/
token families with their consumers. Restricted external authentication is separate selected scope, not a reason
to put those mechanisms back into the local wire.

Change DTOs, codec, dispatcher, client and CLI together. Window insertion/advance follows its existing complete-
output boundary; local flush is not peer acknowledgement. Do not introduce another compatibility wire version.
After code, run minimal pinned compiler/Clippy for actual consumers; runtime claims require execution.
