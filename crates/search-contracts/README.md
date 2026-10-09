# search-contracts

**C00 — Versioned contracts and canonical identity foundation.**

**Status:** P00 contract kernel and closed canonical codec are implemented. The #237 real-digest/bounded-sink foundation remains implementation work; package acceptance, integration and W0/G0 qualification are separate decisions.

This crate defines the bounded, vendor-neutral wire and domain vocabulary shared by every ELIOT Search package. It is also the sole production owner of the closed `CanonicalValue` vocabulary and canonical JSON/CBOR byte encoding.

## Implemented guarantees

- exact closed registries for the eleven v1 recipe/result families, semantic enums, reason codes, protocol messages and lifecycle states;
- strongly typed UUID, epoch, revision, digest, profile, opaque reference and bearer-token wrappers;
- strict P00 limits for strings, bytes, collections, maps, nesting, frames and in-flight requests;
- deterministic canonical JSON and RFC 8949 CBOR with non-canonical, duplicate, unknown, malformed and oversized input rejected;
- strict canonical decode/re-encode checks;
- separate planning (`QuerySnapshotFence`) and emission (`ResultFence`) boundaries;
- validated evidence candidates, explicit ambiguity, coverage gaps and exact-denominator conclusions;
- opaque handles and continuations that carry no source identity or authorization decision;
- bounded provider framing, version negotiation, progress, capability and lifecycle records;
- deterministic fingerprint inputs for query snapshots and task plans.

## Canonical CBOR profile

The current encoder orders map keys by encoded-key length and then bytewise value. This is the RFC 8949 **length-first deterministic ordering** profile described in §4.2.3. It is not the §4.2.1 Core Deterministic ordering profile.

Existing accepted bytes and golden fixtures are compatibility data. Changing ordering requires an explicit new canonical profile and migration; it is not a transparent implementation cleanup.

## Ownership boundary

The crate owns contract shapes, validation, canonicalization and the shared algorithm-qualified digest construction boundary accepted by #237. It performs no filesystem, network, process, secret-store, redb, Qdrant or provider I/O.

Historically the crate had no external dependencies. The reviewed #237 exception permits exact private `blake3` and RustCrypto `sha2` algorithm dependencies only. Donor types, dynamic donor values and donor defaults do not cross the package boundary. Ciborium is not the production codec.

See:

- [package agent contract](AGENTS.md);
- [single-manager Wave-1 packet](../../docs/audit/WAVE1_SINGLE_MANAGER_PACKET_2026-10-09.md);
- [donor verification register](../../docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md);
- [issue #237](https://github.com/UnknownAlienHuman/eliot-search/issues/237).

## Current source gate for #237

```text
cargo +1.98.0 check --locked -p search-contracts -p xtask --all-targets
cargo +1.98.0 clippy --locked -p search-contracts -p xtask --all-targets -- -D warnings
```

Focused canonical/digest/guard fixtures are written with the implementation and compiled by `--all-targets`. Broad/full test execution and installed/native qualification remain separate later phases under the project testing policy.

- **Delivery wave:** W0 / P00
- **Soft source-line target:** 8,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
