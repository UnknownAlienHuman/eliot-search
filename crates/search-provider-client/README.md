# search-provider-client

One concrete current-protocol client owns its connection, bounded I/O and request correlation. It may expose
a synchronous call while keeping its internal control/read path live. Framing, clocks, cancellation and
session semantics have one definition in the [protocol map](../search-provider-protocol/FUNCTIONS.md).

Current source still carries pairing/profile/grant expectations and sequence history while the CLI has a
separate legacy core. Replace their actual callers together; do not add another extraction or adapter.

A terminal result completes one request. A live session retains its child/streams for Continue/Expand;
a one-shot invocation closes the child and returns no reusable window locators. Each operation gets a new
request deadline; window expiry remains fixed. Cleanup is not a renewed work budget.

Keep owned stream/child cleanup and finite response validation. Remove mandatory local pairing/grant/binding
records, legacy loopback, old wire goldens and duplicate CLI session code. A selected restricted client can
authenticate separately; do not prebuild it.

The existing [source map](FUNCTIONS.md) records code to remove; its former API sketches are not obligations to
implement start/cancel/receive/pipe variants without a caller. Use the protocol map for the corrected runtime
semantics and the [CLI map](../../bins/eliot-search/FUNCTIONS.md) for invocation lifetime.
