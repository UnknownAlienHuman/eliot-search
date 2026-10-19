# Working on search-provider-client

Read the [protocol](../search-provider-protocol/README.md),
[implementation guide](../../docs/implementation/README.md) and local [source map](FUNCTIONS.md).

Implement one small synchronous framed client used by the actual CLI. Keep owned connection I/O, finite
buffers, request-ID correlation, optional cancellation/progress, exactly one terminal response, original
absolute deadline and fail-stop partial I/O.

Do not preserve or wrap:

- `NativeClientExpectations`/binding/pairing/profile comparison for child stdio;
- mandatory standalone grant state;
- per-record MAC/sequence/replay transcript history;
- legacy loopback TCP/token/endpoint constructors;
- duplicate CLI `core::ProviderSession`;
- exact old wire bytes or fixture-only consumers.

The child-stdio constructor consumes and owns the child/stream handles. A persistent owner-only pipe is
validated by its platform owner before this package receives it. A selected restricted-client adapter may
authenticate separately, then use the same current framed session; do not fabricate restricted identities for
trusted local use.

Change CLI, current protocol and daemon dispatcher together. Never translate current requests back to
`RecipeIdV1` or legacy tab/hex operations. After code, run minimal Rust 1.98 compiler/Clippy for the package
and real CLI/daemon consumers; delete old grant/profile/wire fixtures with their APIs.
