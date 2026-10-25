# Working on the old Windows provider transport

Read the [current protocol](../../search-provider-protocol/README.md),
[client map](../../search-provider-client/FUNCTIONS.md), [CLI issue #332](https://github.com/UnknownAlienHuman/eliot-search/issues/332), package [README](README.md) and local [source map](FUNCTIONS.md).

The baseline local product uses owned child stdio. Historical audits found this client-only `PIPE_NOWAIT`
adapter coupled to the old typed provider/pairing path and no selected child-stdio consumer. Treat that as
revision-specific evidence. Immediately before removal, search current manifests, Rust and non-Rust callers,
constructors, features, targets, tests and workflows.

When the current search confirms no real persistent-daemon consumer, remove the package/workspace/dependency
entries and old client callers together. If a consumer exists, name it and retain only the smallest truthful
unavailable seam while migrating it; do not harden legacy endpoint, pairing, grant or session APIs.

A future selected persistent-daemon profile may implement one actual owner-only Windows pipe server/client using
the current framed protocol, real ACL/peer-owner validation and one bounded I/O context. Do not preserve
`NativeEndpointNameV1`, endpoint files, pairing keys, grants, capability digests, TCP fallbacks, discovery or
reconnect platforms.

After deletion or replacement, run minimal Rust 1.98 check/Clippy for actual protocol/client/CLI consumers.
Native pipe runtime cases are required only when that optional profile is selected.
