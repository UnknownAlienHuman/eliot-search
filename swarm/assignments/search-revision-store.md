# `search-revision-store` implementation packet

**Path:** `crates/search-source/search-revision-store`  
**Capability:** C07  
**Delivery:** W2 / P04  
**Gate:** BLOCKED until source registry and safe-reader receipts are accepted  
**Trace:** S6.3, S7.3, S15, S24, S28.4, H6, H13, P04  
**Direct public handoffs:** `search-contracts`, `search-domain`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Store and reopen immutable residency-scoped source revisions, manifests and coordinate artifacts required for coherent readback.

## Owns

- residency-key-derived CAS layout
- atomic immutable object writes and reopen verification
- revision retention leases and manifest references
- exact revision/anchor readback primitives and coordinate-map storage

## Must not own

- query/index APIs
- global content-digest-only CAS namespace
- cross-domain ciphertext/key reuse
- refcount-only deletion or purge authority

## Logical primitives

- CasObjectKind, CasObjectAddress, ResidencyClosure, AtomicWriteReceipt, SourceRevisionManifest, RetentionLease, CoordinateMapRef, LossMapRef, RevisionReadback

## Logical operations

1. `derive_object_address(residency_key, kind, content_digest) -> CasObjectAddress`
2. `write_immutable(address, bytes) -> Result<AtomicWriteReceipt, StoreError>`
3. `open_verified_revision(ref) -> Result<RevisionReadback, StoreError>`
4. `retain_revision(ref, owner, expiry) -> Result<RetentionLease, StoreError>`
5. `release_lease(lease) -> Result<(), StoreError>`
6. `resolve_native_anchor(revision, anchor, maps) -> Result<ByteRange, StoreError>`

## Required invariants

- equal bytes share storage only under complete equivalent residency domains
- writes use temporary file, fsync, atomic rename and reopen verification
- visible epochs/handles/jobs retain exact working-tree bytes they cite
- readback verifies digest, length and anchor mapping
- lossy maps cannot claim raw-byte exactness

## Typed failure surface

- `SOURCE_REVISION_UNAVAILABLE`
- `RESIDENCY_DOMAIN_MISMATCH`
- `CAS_OBJECT_CORRUPT`
- `ANCHOR_MAPPING_FAILED`
- `MATERIALIZATION_LOSS`
- `RETENTION_LEASE_CONFLICT`

## Exit tests / evidence

- `cross_domain_physical_dedup_denied`
- `atomic_write_reopen_digest_check`
- `A_B_A_revision_occurrences_remain_distinct`
- `utf8_crlf_utf16_coordinate_fixtures`
- `visible_revision_lease_prevents_gc`
- `lossy_map_never_reports_exact_bytes`

## Suggested internal modules

```text
search-revision-store/src/
  residency.rs
  address.rs
  atomic_write.rs
  manifest.rs
  lease.rs
  readback.rs
  coordinate.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 7,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Keep raw revisions/maps/manifests together while one readback invariant governs them. Mark-and-sweep execution belongs to search-retention.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.
