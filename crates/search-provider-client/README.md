# search-provider-client

Target: one serviced framed session used by the real CLI and daemon. A synchronous public call may wrap it;
active cancellation is required, while progress and additional transport modes are optional.

The current source still has pairing/profile/grant and replay state, and the CLI retains a separate legacy core.
Those are not the target implementation. [FUNCTIONS.md](FUNCTIONS.md) identifies the inspected seams and cutover;
[protocol](../search-provider-protocol/FUNCTIONS.md) owns framing/control/clocks;
[CLI](../../bins/eliot-search/FUNCTIONS.md) owns invocation and presentation.

There is one owned-child session/cleanup guard, not one process manager per command. Remove obsolete callers
with their producer. Do not implement every old API sketch or future transport merely because it was named.
