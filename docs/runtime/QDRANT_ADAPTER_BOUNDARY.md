# Qdrant adapter boundary

Qdrant is the only vector/index database, but it is a replaceable external
backend. The service must depend on Eliot-owned contracts, not on a particular
`qdrant-client` release.

## Dependency direction

```text
daemon / publication / retrieval / reclaim
                    |
                    v
       search-qdrant-bridge public API
                    |
                    v
        private qdrant-client transport
                    |
                    v
          qualified Qdrant server
```

The boundary is strict:

- only `search-qdrant-bridge` may depend on `qdrant-client`;
- raw `qdrant_client::*` types stay inside that package;
- public bridge signatures use Eliot-owned records and errors;
- collection names, protobuf messages, gRPC status text and vendor filters do
  not cross the package boundary;
- the supervisor owns process lifecycle and supplies a qualified endpoint;
- the daemon composes the bridge but does not construct vendor requests.

Run the structural gate from the repository root:

```powershell
cargo run --locked -p xtask -- validate qdrant-boundary --json
```

The gate scans every Cargo manifest and Rust source file. It also requires the
workspace client pin, `Cargo.lock`, `qualified.rs` and
`qualification/qdrant/artifact.toml` to name the same client version, and
requires the qualified server version to match the artifact manifest.

## Upgrade procedure

A Qdrant upgrade is an adapter qualification change, not a service rewrite.

1. Change the single exact `qdrant-client` pin in the root
   `[workspace.dependencies]`.
2. Update `Cargo.lock` with that exact client release.
3. Update the qualified client/server identities in
   `search-qdrant-bridge/src/qualified.rs` and
   `qualification/qdrant/artifact.toml`.
4. Adapt only private translation code under `search-qdrant-bridge` when the
   vendor protobuf/API changed.
5. Run `xtask validate qdrant-boundary`, bridge unit/contract tests and the live
   disposable-server qualification.
6. Update the qualification receipt only from executed evidence.

No daemon, query, publication or control-store module may be changed merely to
accommodate a vendor SDK type or method rename. Such a required change means
the adapter boundary leaked and must be repaired before qualification.
