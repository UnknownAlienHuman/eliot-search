# Daemon live gRPC parity test

Date: 2026-09-15. Base: `16350c4c6fb88628335c1ba0ed4d5529964857c3`.
Tracking: product-test PR #185 and T22/T24/T30 evidence lanes.

## Added lane

`bins/eliot-searchd/tests/qdrant_grpc_parity_process.rs` is a native
Windows/all-features integration test over the public bridge API. It does not
use the daemon's REST-oriented loopback transport as a Qdrant substitute.

The test:

1. finds the exact frozen Qdrant executable (`ELIOT_QDRANT_EXE` override or
   the bridge-owned `NATIVE_EXE_PATH`);
2. spawns it on disposable OS-assigned loopback HTTP/gRPC ports;
3. executes every `MANDATORY_LIVE_PROBES` item through the pinned
   `qdrant-client` transport;
4. admits `QualifiedGate` only from that executed receipt;
5. connects `RealDataPlane`, creates and reads back the strict one-shard sparse
   schema;
6. writes one eligible and two deliberately higher-scoring denied points;
7. proves the exact filtered count is one;
8. executes `IdfScope::ScopedToRetrieval` and proves only the eligible point is
   nominated; and
9. performs exact three-ID readback with no missing or unexpected identifiers.

The denied records cover both independent access-partition and membership
rejection. Their larger sparse weights make a filter leak visible in ordering,
not merely in a zero-score edge case.

## Boundaries

The test crate is compiled only for `windows + wave3-index`. A Linux build does
not pretend to execute the qualified Windows artifact. The added Tokio entry is
a test-only exact pin already present in `Cargo.lock`; it does not add a daemon
runtime, production transport or another package.

This is evidence code, not evidence itself. It does not edit
`qualification/qdrant/artifact.toml`, manufacture `QualifiedGate`, publish a
capability snapshot or authorize normal startup. The disposable server and gate
exist only inside the test.

## Required execution

```powershell
cargo +1.98.0 test --locked -p eliot-searchd --all-features \
  --test qdrant_grpc_parity_process -- --nocapture
```

Then run the reference adapter suite as a separate owner-level check:

```powershell
cargo +1.98.0 test --locked -p search-qdrant-bridge \
  --test real_dataplane -- --nocapture
```

Both commands, exact executable digest, process cleanup and the 13 probe
outcomes remain **NOT_RUN** in the current tool environment. No T22, T24 or T30
`PASS` is claimed by this source increment.
