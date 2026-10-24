# Working on the old Windows provider transport

Read the [current protocol](../../search-provider-protocol/README.md),
[client map](../../search-provider-client/FUNCTIONS.md), [CLI issue #332](https://github.com/UnknownAlienHuman/eliot-search/issues/332) and local [source map](FUNCTIONS.md).

Do not harden or extend this client-only `PIPE_NOWAIT` adapter. It is coupled to the deleted pairing/grant
provider stack, has no server/listener and is not used by the selected child-stdio product path. Remove the
package/workspace member with its last old typed-client/CLI callers.

A later persistent-daemon feature may implement one actual owner-only Windows pipe server and client against
the current framed protocol. It must validate real ACL/peer ownership and use one bounded I/O context. Do not
preserve `NativeEndpointNameV1`, endpoint files, pairing keys, grants, capability digests or TCP fallbacks.

After deletion, run minimal Rust 1.98 compiler/Clippy for actual client/CLI consumers. Native pipe tests return
only with a real selected persistent-pipe implementation.
