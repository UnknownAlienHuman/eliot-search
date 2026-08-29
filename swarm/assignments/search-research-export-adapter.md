# `search-research-export-adapter` implementation packet

**Path:** `crates/search-research-export-adapter`  
**Capability:** C30 optional Research profile  
**Delivery:** W8 / optional P14  
**Gate:** OPTIONAL and disabled by default; start only after generic edge and retained-readback contracts are accepted  
**Trace:** S1.4, S7.2.1, S32.4, H16.6, P14  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-provider-protocol`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Export a qualified immutable Search materialization as the exact eliotr.normalized.v1 bundle and validate ownership-cutover receipts when requested.

## Owns

- normalized manifest/archive assembly
- independent wire SHA-256 verification from retained native bytes
- ownership_mode validation and cutover receipt verification
- export receipt and disclosure/residency checks

## Must not own

- online research orchestration or conclusions
- transferring ownership through ordinary export
- relabeling internal BLAKE3 as wire SHA-256
- exporting unsaved overlay without snapshot admission
- cross-domain CAS/key reuse

## Logical primitives

- NormalizedBundleManifest, ExportRequest, ExportOwnershipMode, ExportContentSet, WireDigestSet, CutoverReceiptValidation, ExportReceipt

## Logical operations

1. `prepare_export(request, retained_materialization) -> Result<ExportPlan, ExportError>`
2. `reopen_and_verify_native_bytes(plan) -> Result<VerifiedExportContent, ExportError>`
3. `compute_wire_sha256(content) -> WireDigestSet`
4. `validate_ownership_mode(manifest, optional_receipt) -> Result<(), ExportError>`
5. `assemble_normalized_bundle(plan, content) -> Result<ExportReceipt, ExportError>`

## Required invariants

- manifest shape/digest matches eliotr.normalized.v1
- unknown load-bearing fields fail closed
- ordinary export is immutable import candidate, not owner transfer
- ownership_cutover requires completed valid source.owner-cutover.v1 receipt
- receipt field absent for other modes
- unsaved content requires durable admitted snapshot

## Typed failure surface

- `EXPORT_PROFILE_DISABLED`
- `EXPORT_SOURCE_UNAVAILABLE`
- `EXPORT_DIGEST_MISMATCH`
- `OWNERSHIP_CUTOVER_RECEIPT_INVALID`
- `UNSAVED_SNAPSHOT_NOT_ADMITTED`
- `DISCLOSURE_NOT_AUTHORIZED`

## Exit tests / evidence

- `manifest_body_digest_golden`
- `native_blake3_to_wire_sha256_independent`
- `unknown_field_fail_closed`
- `ordinary_export_no_ownership_transfer`
- `cutover_mode_exact_receipt_validation`
- `unsaved_export_rejected`
- `cross_domain_reuse_rejected`

## Suggested internal modules

```text
search-research-export-adapter/src/
  manifest.rs
  plan.rs
  readback.rs
  digest.rs
  ownership.rs
  archive.rs
  receipt.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 6,000 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Keep as one leaf adapter while one wire archive/ownership contract governs it.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.
