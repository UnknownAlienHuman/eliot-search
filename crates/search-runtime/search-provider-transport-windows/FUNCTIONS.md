# Windows pipe transport — legacy boundary

The selected [local product](../../../docs/architecture/LOCAL_SCOPE.md) uses one owned-child stdio connection,
not a separately listening pipe service or restricted-client protocol. Do not add pairing, credentials, endpoint
discovery, new transport variants or compatibility constructors here.

<a id="current-api"></a>
<a id="current-disposition"></a>
<a id="optional-future-pipe"></a>
<a id="verification-if-reintroduced"></a>
## Cutover

Main's Windows CLI still declares this package; move its actual client/dispatcher consumers before removing
source/Cargo/features/fixtures. Preserve any useful OS I/O handling at its real owner, not a forwarding crate.
[Client](../../search-provider-client/FUNCTIONS.md) and [protocol](../../search-provider-protocol/FUNCTIONS.md)
own the replacement. Run minimal compiler/Clippy including the Windows consumer when source changes.
[Prior recipe](https://github.com/UnknownAlienHuman/eliot-search/blob/6ae24197a1be8695ede5bb7d12dfaf97f5bca1fc/crates/search-runtime/search-provider-transport-windows/FUNCTIONS.md) is historical. No transport
implementation or package deletion is delivered by this documentation change.
