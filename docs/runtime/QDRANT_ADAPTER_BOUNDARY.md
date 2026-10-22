# Qdrant adapter boundary

Qdrant is the only indexed backend. `search-qdrant-bridge` is the sole package allowed to depend directly on `qdrant-client`; vendor request, response, filter and schema types remain private.

## Runtime boundary

- daemon composition supplies one configured endpoint and optional actual credential;
- publication and query use the same concrete adapter and collection generation;
- startup observes the current endpoint, server capabilities and selected schema;
- past probe output, a matching version string or an executable checksum is not runtime authority;
- mutation uncertainty is returned to publication for reconciliation; reads do not create commit ambiguity;
- managed-child ownership is optional and does not change the bridge API.

Use Cargo metadata, Rust privacy, public-API review and focused behavior checks. The former custom Rust source analyser and its generated boundary database were deleted; do not replace them with another parser or import graph.

## Current code debt

The public source still contains `qualified::*`, a live probe suite and `RealDataPlane::connect` requiring a `QualifiedGate`. This is a remnant of the former qualification operating system. It must be reduced in the #263 bridge cutover to direct construction from:

```text
selected endpoint + actual credential + current capability/schema observation
```

Preserve useful current probes and exact mutation/readback behavior. Remove static receipt chains, fixture admission and runtime dependency on historical qualification records. This document records the cut; it does not claim the code is already simplified.

## Optional process owner

`search-qdrant-supervisor` currently has no connected product caller. A future managed-child mode may own an actual process handle, private config, finite startup probe and bounded shutdown. It must not block external-endpoint Search or manufacture readiness from caller-supplied receipts.

## Verification

For a bridge change, compile the bridge and real consumers, then execute the specific schema, query, mutation or recovery case affected. Record raw results with the owning issue or PR. Do not store them as credentials required by ordinary startup.
