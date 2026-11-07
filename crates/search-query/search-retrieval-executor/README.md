# Retrieval execution — retired Rust package

Q1 removed the disconnected scheduler/leg/receipt executor source, manifests and exclusive fixtures.
Current ranked search comes directly from the daemon's concrete Backend call and
[verified_query.rs](../../../bins/eliot-searchd/src/verified_query.rs), which resolves pinned evidence and returns
verified retained excerpts. There is no separate executor package to connect.

[FUNCTIONS.md](FUNCTIONS.md) retains direct query execution/rank/failure policy for the connected daemon/
CONTROL/readers, preserving backend order and original deadline/cancellation.
Do not resurrect the old process-test oracle or executor platform.
