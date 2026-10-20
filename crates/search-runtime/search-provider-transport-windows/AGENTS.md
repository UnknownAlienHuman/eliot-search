# Working on Windows provider transport

This package is optional. Read the [local protocol](../../search-provider-protocol/README.md),
[CLI cutover](../../../bins/eliot-search/FUNCTIONS.md) and local [boundary](FUNCTIONS.md).

Implement child stdio first under #332. Keep this package only if a persistent owner-only Windows pipe is an
actual selected product route.

If retained, own one admitted non-clone pipe handle and synchronous byte I/O only. The platform/connection
owner establishes real user/installation identity before handing the endpoint to this package. Pipe name,
reachability and byte mode are not authentication.

Use the original absolute deadline/cancellation, preserve partial counts, distinguish would-block/EOF and
keep diagnostics content-free. Do not add pairing, grant, protocol parsing, endpoint discovery, TCP/token
fallback, arbitrary handles, listener/process lifecycle, reconnect, pooling or receipts.

If no persistent pipe consumer remains after child-stdio integration, delete the package and its native
registration/listener callers rather than preserving a second transport. After code run minimal Rust 1.98
compiler/Clippy for actual consumers and native checks only for the selected profile.
