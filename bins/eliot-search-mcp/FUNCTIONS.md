# MCP stdio adapter

This binary is the MCP boundary, consuming search-provider-client directly. It
does not execute the CLI, open CONTROL, initialize roots, publish, start Qdrant,
or reinterpret the daemon's private u32-length protocol as MCP.

    eliot-search-mcp --data-root ROOT [--daemon PATH] [--qdrant-url URL]

ROOT must be absolute. The default daemon is the sibling eliot-searchd executable.
Only ranked operations require the configured loopback Qdrant URL. Native paths
remain native arguments. Startup, initialize/discovery and tools/list open no root.

## Actual transport and tools

rmcp is pinned to 3.9.0 with server/macros/transport-io; its reexported schemars
generates closed typed input schemas. Stdout is MCP JSON-RPC only; diagnostics
and explicit --help/--version output use stderr. No custom MCP parser, HTTP
listener, negotiation layer or host configuration writer is installed.

- eliot_search: query, optional limit (default 5, legal 1..64), timeout_ms
  (default/max 120000), include_source_info (default true).
- eliot_exact: mode raw/decoded_utf8, literal, optional source_ids, limit,
  timeout_ms and include_source_info. Omitted/null source_ids selects all
  published sources; [] selects none. UUIDs are canonical and unique.
- eliot_window_open: query in the existing WindowQuery mode/request shape,
  page_size (default 16, 1..64), ttl_ms (default/max 120000), timeout_ms and
  include_source_info.
- eliot_window_continue: window_handle, page_size and timeout_ms.
- eliot_window_expand: window_handle, item_index, explicit before_bytes and
  after_bytes, timeout_ms. SDK validation enforces emitted membership, the
  original range/representation, UTF-8 boundaries and total context <=65536.
- eliot_window_cancel: window_handle and timeout_ms. Cancels correlated active
  work without closing a healthy emitted prefix. If no request is active, it
  reports cancel_requested=false; a race reports the actual observed outcome.
- eliot_window_close: window_handle and timeout_ms. Closes all context and
  observes SDK finish/reap before a successful response.

Query/literal UTF-8 byte bounds, source selection, framing and evidence validation
remain at the existing protocol/SDK boundary. The small UUID-count preflight is
derived from the existing core request-frame byte ceiling, not a new corpus quota.
The nested query limit also defaults to 5. Unsupported selectors are not added to
ranked Search.

## Evidence, summary and saved source hints

One-shot results include the entire actual validated Terminal: evidence items,
full unit text or exact bytes_hex, immutable citations, scan/gap/completeness facts
and the pinned corpus summary. No statistics-only replacement, lossy byte decode,
live-path fallback or unverified text truncation is used.

The MCP result contains one text ContentBlock holding the FULL validated SDK JSON.
There is no second full structuredContent copy. All supported clients can consume
this text-compatible result. Core Failed is a tool error with its typed terminal;
metadata Unavailable inside a valid summary is still a valid evidence result.
Core Cancelled remains a typed outcome, not a CLI exit 130 or fabricated absence.

The MCP sets QueryClient::set_include_source_info(true) by default and forwards
the corresponding flag on Open. SourceInfo is preserved from the core's same-view
captured-source projection. saved_path_hint is retained location metadata;
live_status=not_checked is not a claim that the file still exists or is unchanged.
Annotation unavailability does not replace valid evidence. There is no second
source-info RPC or CONTROL helper in this adapter.

## Ownership, cancellation and shutdown

One ordinary joinable thread runs synchronous QueryClient calls outside Tokio's
entered context. A fresh QueryClient/runtime and owned daemon are used per call;
the SDK validates the full reply, drains both output EOFs and observes child
wait/reap before the tool result is produced. Tokio spawn_blocking is not used to
construct QueryClient. The idle thread holds no root or child.

One joined asynchronous window owner holds OwnedQuerySession only after explicit
Open, services data and separate bounded Cancel control, and polls next_reply
during idle expiry. One process-local window per configured root is admitted;
other local rooted work refuses Busy rather than queuing unbounded work. Handles
are opaque epoch-plus-sequence strings: w followed by 32 lowercase OS-random hex
digits, a hyphen and the nonzero sequence. A new process/stdio connection obtains
a fresh epoch before workers start. Handles have no durable/replay or RootId meaning.

The original SDK window TTL is not renewed. Only an explicit window retains the
root between tools. Close bypasses the data queue; Cancel does not impersonate
Close. MCP RPC cancellation/deadline/abandonment closes its window and awaits
settled cleanup, whereas the explicit window_cancel tool preserves healthy
context when the correlated core cancellation does so. SDK local command
rejection preserves a healthy window.

A bounded raw stdin pump observes EOF independently of rmcp's writer/close mutex.
It does not parse MCP. Exhausting its existing-16MiB-derived byte queue fails the
connection instead of silently dropping input and continuing. EOF/output failure
signals cancellation, then the actual query-thread join and SDK window finish/reap
run without awaiting blocked MCP output. Only after successful product cleanup may
standard-stream transport tasks and Tokio blocking-stdio teardown be time-bounded.
A timeout/drop is never evidence that product children exited. Cleanup failure
remains failure and supplies no positive bounded-teardown claim.

Every outgoing SDK JSON-RPC message is measured with the same Serde serializer as
rmcp's codec, including the real ID, result/error, repeated text escaping and final
newline. The existing 16MiB RPC ceiling is not raised. A valid full tool response
that cannot fit returns isError=true with text JSON {"error":"MCP_RESPONSE_TOO_LARGE"}
after the affected window has closed and SDK cleanup settled. No item/page clipping,
completeness downgrade or fake partial success is produced. If even that error with
the same ID cannot fit, the connection fails after product cleanup rather than
emitting an oversized or malformed line.

One ephemeral pending window-data delivery slot holds the real request ID and
cleanup binding through the actual SDK send/flush. Further window data refuses
MCP_WINDOW_DELIVERY_IN_FLIGHT while that response is outstanding; Cancel and Close
remain independently serviceable. A real cancellation notification after private
window advance closes that exact window. If rmcp suppresses the response before
transport send starts, the notification callback itself awaits actual window
cleanup and then releases delivery admission. The send-start transition and
cancellation use the same mutex; once send has started, only the transport releases
admission after settlement or connection fail-stop. A potentially partial outer write fails
this connection; no error is appended to an uncertain JSON line. Local flush is
not peer acknowledgement. An uncertain delivery offers no replay or skipping of
an unseen page. Qdrant and foreign processes are never terminated.

## Checks

After CORE_HINTS_READY and a common source freeze, the integration owner runs the
smallest locked check/strict Clippy for this bin, protocol/client and actual
CLI/daemon consumers. Native MCP handshake/tool/evidence, Cancel/Close/TTL/EOF
and client-registration qualification are separate execution evidence. This
recipe does not claim those gates or installed client configurations passed.
