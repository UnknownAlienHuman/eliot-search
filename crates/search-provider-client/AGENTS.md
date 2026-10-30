# Working on search-provider-client

Read the [architecture](../../docs/architecture/README.md), the
[protocol recipe](../search-provider-protocol/FUNCTIONS.md), and [FUNCTIONS.md](FUNCTIONS.md).

Implement one serviced framed session used by the actual CLI and daemon. The public call may be synchronous;
the connection must still service response reads, partial writes, cancellation and its deadline while work is
active. Cancellation is baseline behavior, not an optional future API. Progress is optional.

Use one active data request and bounded control state. Do not retain completed-request replay history, add a
multiplexing scheduler or let a data/history limit prevent cancellation and close. Never interleave a control
frame into a partially written frame; follow the protocol owner's failure rule.

The child-stdio session owns exactly one child/stream cleanup guard. The CLI supplies invocation settings and
renders results; it does not build another process manager. Only owned-child stdio is selected. Do not preserve
a named-pipe/endpoint connection mode, and never kill the independently operated Qdrant server.

Replace pairing/profile/grant authority, NativeClientExpectations, per-record MACs, replay transcripts,
loopback/token/endpoint fallback and duplicate CLI core sessions with their actual consumers. Preserve useful
framing, fixed-deadline polling, typed error/result distinction and fail-stop cleanup. Do not translate new
requests through old RecipeIdV1 or tab/hex messages.

After a production cut, run minimal pinned compiler/Clippy for the actual CLI/client/protocol/daemon targets.
Neither a constructor nor a local documentation change establishes a working transport.
