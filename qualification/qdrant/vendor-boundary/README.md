# Qdrant vendor-locality boundary

Run:

```powershell
cargo test --locked -p xtask --test qdrant_vendor_locality
cargo run --locked -p xtask -- validate qdrant-boundary --json
```

The workspace boundary command prevents `qdrant-client` dependencies and SDK types from escaping `search-qdrant-bridge`. The narrower test keeps SDK/protobuf references inside the bridge's private `real/**` transport and `live/**` qualification-probe code.

The vendor-neutral model, in-memory oracle, qualification identity, daemon, publication, query, retention and control-store code must remain independent of Qdrant SDK spelling. A server/client upgrade that forces those owners to change is a boundary failure.

These structural checks do not replace disposable-server live qualification or executed receipts.
