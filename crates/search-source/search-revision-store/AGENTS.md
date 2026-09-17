# Agent contract — search-revision-store

You own only `crates/search-source/search-revision-store/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S6.3-S6.4, S7.5, S15, S28.4, H6, P04.

## Mission

Admit, retain and reopen immutable source revisions under complete residency identities.

## Ownership

- residency-key-derived CAS paths
- no-clobber temporary write, file/directory sync and immutable publication
- bounded exact object reads and stable native-identity/locator fencing
- raw revision and manifest integrity
- retention leases and exact reopen
- copy/re-encrypt transition receipts
- bounded legacy DIRECT `.bin` / `.dpapi` object I/O during T02 migration
- closed legacy revision root/shard/final/temporary filename grammar
- stable physical inventory classification tags and canonical relative locators
- deterministic legacy inventory ordering, counts and names/sizes/mtimes digest
- legacy orphan checkpoint, canonical cursor, bounded page and exact JSON projection

## Forbidden ownership

- query language or ranking
- global content-digest-only CAS namespace
- cross-domain co-residency, ciphertext or key reuse
- source identity or access authorization
- secret acquisition, DPAPI/keyring operations or plaintext verification policy
- preparation/materialization artifacts owned by `search-materializer`
- data-root traversal, metadata acquisition or object-content hashing

## Allowed dependencies

`search-contracts`, `search-domain`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `derive_object_path(residency_key, kind, digest) -> CasPath`
- `admit_revision(stable_read, residency) -> Result<RevisionReceipt, CasError>`
- `reopen_exact(revision_ref) -> Result<VerifiedRevision, CasError>`
- `retain(revision_ref, cause) -> Result<RetentionLease, CasError>`
- `copy_or_reencrypt(source, target_residency) -> Result<TransitionReceipt, CasError>`
- `enumerate_mark_roots(snapshot) -> MarkRootSet`
- `read_legacy_revision_object(platform, path, maximum_bytes)`
- `publish_legacy_revision_object(platform, directory, final_name, temporary_name, bytes, maximum_bytes)`
- `classify_legacy_revision_inventory_name(name)`
- `legacy_revision_inventory_relative_locator(shard, name)`
- `build_legacy_revision_inventory(shards, observations)`
- `legacy_revision_inventory_checkpoint(catalog, backend, inventory)`
- `plan_legacy_revision_inventory_page(inventory, checkpoint, cursor)`
- `render_legacy_revision_inventory_report(namespace, catalog, inventory, page, evidence)`

The legacy adapter treats bytes as opaque and accepts qualified native identity,
locator and directory-durability observations through an injected platform. It
must never infer source identity, decrypt/protect bytes, authorize a revision or
write a preparation artifact. The pure inventory owner accepts only normalized
qualified observations plus catalog-overlay classifications. Concrete SHA-256,
filesystem traversal, metadata and content fingerprints remain injected by daemon
composition. A racing final object is reused only after exact readback; a
conflicting object is never replaced.

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `SOURCE_REVISION_UNAVAILABLE`, `RESIDENCY_DOMAIN_MISMATCH`, `CAS_INTEGRITY_MISMATCH`. Never turn a degraded or partial
state into an apparent success. Possible publication followed by failed durability is outcome-unknown, not an ordinary retryable miss.

Legacy inventory construction and paging preserve the existing bounded
`DIRECT_MIGRATION_*` reason namespace. Malformed cursors are rejected before
catalog or filesystem access. A stale checkpoint, zero-progress page, aggregate
byte overflow or overlarge report fails closed and never authorizes deletion.

## Test seams and exit evidence

- `same bytes different residency keys produce different objects`
- `atomic-write crash leaves no admitted partial object`
- `reopen verifies residency digest, content digest and length`
- `cross-domain physical/ciphertext/key reuse denied`
- `visible epoch or durable handle keeps revision reachable`
- `legacy immutable object exact replay reuses without replacement`
- `legacy immutable object conflict preserves existing bytes`
- `legacy read rejects changed native identity and oversize before allocation`
- `DIRECT revision paths compose this package while preparation artifacts do not`
- `legacy revision final/temporary names and shard grammar fail closed`
- `inventory order/digest/cursor/page/report bytes remain frozen`
- `daemon contains no duplicate revision filename parser, inventory model, digest domains or report schema`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W2 / P04**
- Soft `src/` target: **8,000 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.
