# search-provider-protocol

One bounded typed exchange between the current client and actual daemon handlers. The
[implementation map](FUNCTIONS.md) is the single definition of framing, cancellation, deadlines and session
lifetime. Read it with the [project implementation](../../docs/implementation/README.md).

Retain prefix-before-allocation framing, typed Serde records, request correlation and fail-stop partial I/O.
Remove the parallel recipe/grant/pairing/manual-codec/replay systems when their callers move.

Child stdio supports both a one-shot request and a live multi-request session through the same wire. Only a
live session can return reusable window locators. One active request does not mean the control reader may
stop receiving Cancel/EOF. A sync API and a clock check are not an interruptible-I/O implementation.

Do not reproduce the FUNCTIONS algorithm or enumerate future DTOs here. Optional clients/features add code
only with a real selected consumer. No protocol version, dependency or product behavior was changed by this
documentation correction.
