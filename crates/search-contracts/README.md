# search-contracts

**C00 — Versioned contracts.**

**Status:** P00 contract kernel implemented; package acceptance and W0/G0 remain separate integration decisions.

This dependency-free crate defines the bounded, vendor-neutral wire and domain vocabulary shared by every ELIOT Search package.

## Implemented guarantees

- exact closed registries for the eleven v1 recipe/result families, semantic enums, reason codes, protocol messages and lifecycle states;
- strongly typed UUID, epoch, revision, digest, profile, opaque reference and bearer-token wrappers;
- strict P00 limits for strings, bytes, collections, maps, nesting, frames and in-flight requests;
- deterministic canonical JSON and RFC 8949 CBOR with non-canonical, duplicate, unknown, malformed and oversized input rejected;
- separate planning (`QuerySnapshotFence`) and emission (`ResultFence`) boundaries;
- validated evidence candidates, explicit ambiguity, coverage gaps and exact-denominator conclusions;
- opaque handles and continuations that carry no source identity or authorization decision;
- bounded provider framing, version negotiation, progress, capability and lifecycle records;
- deterministic fingerprint inputs for query snapshots and task plans.

## Ownership boundary

The crate owns only contract shapes, validation and canonicalization. It performs no filesystem, network, process, secret-store, redb, Qdrant or provider I/O and has no external dependency.

## Package gate

```text
cargo fmt --all -- --check
cargo check -p search-contracts --all-targets --locked
cargo test -p search-contracts --all-targets --locked
cargo clippy -p search-contracts --all-targets --locked -- -D warnings
cargo doc -p search-contracts --no-deps --locked
```

- **Delivery wave:** W0 / P00
- **Soft source-line target:** 8,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
