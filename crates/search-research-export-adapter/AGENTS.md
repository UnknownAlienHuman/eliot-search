# Agent contract — search-research-export-adapter

You own only `crates/search-research-export-adapter/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S7.2.1, S32.4, H16.6, P14.

## Mission

Export qualified durable materializations through the exact eliotr.normalized.v1 wire bundle and validate ownership-cutover receipts.

## Ownership

- manifest assembly
- native BLAKE3 readback and independent wire SHA-256
- ownership mode validation
- source.owner-cutover.v1 receipt validation
- unknown-field fail-closed behavior

## Forbidden ownership

- unsaved overlay export without durable admission
- relabeling internal digests as wire SHA-256
- transferring ownership by ordinary export
- cross-domain CAS/key reuse
- Research canonical writes
- opening CAS/redb/Qdrant directly; exact bytes are supplied through the daemon-owned export port

## Allowed dependencies

`search-contracts`, `search-domain`, `search-provider-protocol`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `export_normalized_bundle(request, source) -> Result<NormalizedBundle, ExportError>`
- `compute_wire_hashes(reopened_bytes) -> WireDigests`
- `validate_ownership_mode(manifest, receipt) -> Result<(), ExportError>`
- `validate_cutover_receipt(receipt, source_state) -> Result<(), ExportError>`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `EXPORT_SCHEMA_MISMATCH`, `SOURCE_OWNER_CUTOVER_REQUIRED`, `UNSAVED_SNAPSHOT_NOT_ADMITTED`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `exact eliotr.normalized.v1 manifest fixture`
- `wire SHA-256 is computed independently from reopened bytes`
- `unknown load-bearing fields fail closed`
- `ownership_cutover requires exact completed receipt`
- `receipt field absent for federated_reference and immutable_import`
- `unsaved buffers cannot be exported`
- `adapter has no concrete storage dependency`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W8 / optional P14 profile**
- Soft `src/` target: **6,000 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Gate

This package is optional. Do not implement or enable it before the stated gate and ADR.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.
