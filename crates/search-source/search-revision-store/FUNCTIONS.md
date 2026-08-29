# Function contract — `search-revision-store`

**Status:** W2/P04 base contract with W7/P13 lifecycle hardening; no CAS implementation exists yet.

This package owns immutable, residency-aware Search source revisions, coordinate artifacts, manifests and
exact readback. It does not decide retention deletion, purge policy or restore admission. Lifecycle
owners consume its vendor-neutral ports and exact receipts.

## State ownership

The package owns:

- residency-domain derivation and object-address schema;
- immutable CAS object/manifest write, reopen and integrity verification;
- `SourceRevision` occurrence records and retention leases;
- materialization/coordinate/loss-map refs needed for exact source-backed readback;
- exact object/revision inventory and graph edges exposed to lifecycle ports;
- tombstone/quarantine enforcement at the store boundary;
- object deletion execution only when authorized by an exact lifecycle plan/operation.

It does not own source identity, source admission, legal-hold/retention decisions, mark/sweep reachability,
purge fence creation, publication, Qdrant deletion, handle authorization or backup-provider policy.

## `derive_residency_closure`

```text
derive_residency_closure(policy_bindings, encryption_domain, retention_domain, erasure_domain)
    -> Result<ResidencyClosure, StoreError>
```

Produces the complete compatibility identity required for physical deduplication. Equal content digest
alone is insufficient. Any mismatch in access/confidentiality/encryption/retention/erasure domain keeps
objects physically distinct.

Pure, deterministic and versioned.

## `derive_object_address`

```text
derive_object_address(residency, object_kind, content_digest, schema_version)
    -> Result<CasObjectAddress, StoreError>
```

Uses domain-separated deterministic canonical encoding. Address contains no source path/display name and
cannot collide across incompatible residency closures. Unsupported kind/version or zero/invalid digest
fails closed.

## `write_immutable_object`

```text
write_immutable_object(address, bytes, expected_digest, limits, deadline, cancel)
    -> Result<AtomicObjectWriteReceipt, StoreError>
```

**Preconditions**

- bytes are bounded and caller-supplied; the store performs no source acquisition;
- address and digest match object kind/residency/schema;
- target data root is owned and not restore-quarantined for serving writes;
- purge tombstone does not prohibit the object/scope from re-entering Search state.

**Postconditions**

- writes a private temporary object under the same volume/root;
- flushes/fsyncs according to accepted policy;
- atomically publishes only after byte length/digest verification;
- reopens and verifies the final object before success;
- equal existing verified object returns an idempotent receipt;
- conflicting existing object quarantines and never overwrites.

Timeout/cancellation after possible atomic rename becomes `OBJECT_WRITE_OUTCOME_UNKNOWN`; exact reopen
classifies committed/not-applied/conflict before retry.

## `commit_source_revision`

```text
commit_source_revision(command, object_receipts, occurrence, manifests, control_port, operation)
    -> Result<SourceRevisionCommitReceipt, StoreError>
```

Validates exact source namespace/owner generation, occurrence sequence, content digest/length,
residency closure, raw/derived object refs, materialization/coordinate/loss maps and admission receipt.

Durably commits an immutable occurrence record through a control port after all object readbacks succeed.
`A → B → A` creates distinct occurrence revisions even when content bytes/digest repeat. Same operation
identity plus equal command is idempotent; conflicting reuse is rejected.

## `open_verified_object`

```text
open_verified_object(address, expected, budget, cancel)
    -> Result<VerifiedObjectRead, StoreError>
```

Checks data-root/residency/tombstone/quarantine policy before opening. Revalidates object kind/schema,
length and digest. Partial/short/corrupt reads fail closed and may quarantine the object. Returns bounded
bytes behind an owned read guard; default diagnostics contain only object/digest/location class.

## `open_verified_revision`

```text
open_verified_revision(revision_ref, current_authorization, requirements, budget, cancel)
    -> Result<RevisionReadback, StoreError>
```

Requires exact source namespace/owner generation/revision occurrence, current residency permission,
retention/lease state and no purge tombstone. It never resolves a current path or substitutes a newer
revision.

Readback binds source/revision IDs, content digest/length, residency closure, object receipts,
materialization/coordinate/loss-map identities and assurance ceiling.

## `resolve_native_anchor`

```text
resolve_native_anchor(revision, representation, anchor, coordinate_maps, requirements)
    -> Result<VerifiedNativeRange, StoreError>
```

Validates anchor/revision/representation/profile/map digests and bounds. Raw-byte exact range is returned
only when the mapping is lossless for that claim. Lossy/ambiguous mapping returns explicit lowered
assurance or failure; it never fabricates exactness.

## `acquire_retention_lease`

```text
acquire_retention_lease(target, owner, purpose, expiry, policy, control_port, operation)
    -> Result<RetentionLease, StoreError>
```

Creates a bounded durable lease for accepted reasons such as visible projection, durable handle,
verification job, import/export contract or recovery manifest. It binds exact object/revision/residency
and owner/purpose/generation. It cannot target unsaved bytes.

Same operation/equal lease is idempotent. Purged/tombstoned/incompatible-residency targets are rejected.
A lease does not grant source access.

## `release_retention_lease`

```text
release_retention_lease(lease, owner, control_port, operation)
    -> Result<LeaseReleaseReceipt, StoreError>
```

Idempotent for exact lease/owner/generation. Release only removes one root; it never immediately deletes
objects or claims they are unreachable. Mark/sweep decides later.

## `enumerate_lifecycle_roots`

```text
enumerate_lifecycle_roots(control_snapshot, requested_kinds, limits, cancel)
    -> Result<BoundedLifecycleRootPage, StoreError>
```

Exposes exact Search-owned revision/object/manifest roots and graph refs for the captured control
generation through a vendor-neutral port. It does not compute reachability or filter out architecture-
required root kinds. Pagination is deterministic and binds continuation/inventory generation.

Unreadable/corrupt/incomplete root state is explicit and blocks a complete lifecycle root set.

## `enumerate_object_inventory`

```text
enumerate_object_inventory(residency_scope, inventory_generation, limits, cancel)
    -> Result<BoundedObjectInventoryPage, StoreError>
```

Returns exact object addresses/kinds/lengths/digests/residency classes and deletion/quarantine state,
never source bodies. Continuation binds one immutable inventory generation. A changing inventory
returns stale/incomplete rather than mixing generations.

## `read_object_graph_edges`

```text
read_object_graph_edges(object_or_manifest, expected_digest, limits, cancel)
    -> Result<ObjectGraphEdges, StoreError>
```

Parses only versioned Search-owned manifest schemas and returns exact child refs. Unknown/corrupt schema,
digest mismatch, excessive depth/count or cross-residency illegal edge fails closed. It executes no
embedded content.

## `apply_exact_object_deletion`

```text
apply_exact_object_deletion(plan_receipt, exact_addresses, lifecycle_authority, operation, deadline, cancel)
    -> Result<ObjectDeletionReceipt, StoreError>
```

Executes an exact bounded deletion batch only when the lifecycle authority binds an accepted sweep or
security-purge plan, control/root/mark/tombstone generations, residency domain and exact object set.

Broad directory/prefix/content-digest deletion is forbidden. Before dispatch, the store rechecks object
identity and no conflicting quarantine/hold receipt. After dispatch, exact readback verifies absence or
accepted store deletion state.

Timeout/cancellation after dispatch is `OBJECT_DELETE_OUTCOME_UNKNOWN`; readback classifies complete,
none-applied, partial or conflicting. Same operation identity cannot target different addresses.

The receipt identifies authority kind:

```text
ordinary_sweep
security_purge
```

It does not claim backup deletion or physical secure erase.

## `install_purge_tombstone`

```text
install_purge_tombstone(tombstone, control_receipt, operation)
    -> Result<StoreTombstoneReceipt, StoreError>
```

Stores/activates the non-content tombstone generation at the revision/object admission boundary. Any
later write/import/restore/reindex attempt for the tombstoned scope/owner generation is rejected before
object publication. Tombstone removal is not a public operation.

## `enter_restore_quarantine`

```text
enter_restore_quarantine(manifest_ref, target_root, operation)
    -> Result<StoreRestoreQuarantineGuard, StoreError>
```

Marks restored objects/manifests as non-serving and prevents ordinary revision readback/admission until
an accepted restore revalidation/admission receipt names them. Quarantine guard is durable and survives
restart.

## `validate_restored_objects`

```text
validate_restored_objects(quarantine, paired_manifest, tombstone_generation, limits, cancel)
    -> Result<RestoredObjectValidation, StoreError>
```

Verifies every referenced object/manifest identity, digest, length, residency closure, schema and graph
edge against the paired recovery manifest. Purge tombstones dominate: matching material is excluded and
reported, not restored.

Success is only object integrity; lifecycle/registry/access/publication owners still decide admission.

## `admit_restored_objects`

```text
admit_restored_objects(quarantine, lifecycle_admission_receipt, publication_receipts, operation)
    -> Result<StoreRestoreAdmissionReceipt, StoreError>
```

Moves only exact validated objects/manifests into serving eligibility after lifecycle/source/access and
new publication/route receipts are accepted. It cannot resurrect old visible epoch or bypass current
owner/residency/tombstone state. Partial admission remains quarantined.

## Configuration functions

For `revision_store`:

```text
section_descriptor()
compiled_defaults()
validate_section(section)
section_digest(section)
plan_section_change(old, new)
```

Only declared bounded quota/scheduling changes may apply live and never delete immediately. Fsync,
atomic publish, reopen verification, residency separation, tombstone enforcement and quarantine are
locked. Lower quota produces future retention pressure, not direct unsafe deletion.

## Cancellation, deadlines, idempotency and crash semantics

- address/residency/anchor validation is pure/deterministic;
- write/delete/control uncertainty is resolved by exact object/operation readback;
- immutable object publication is atomic or explicitly unknown/conflicting;
- lease release never implies deletion;
- sweep/purge deletion authority and exact sets are revalidated before each batch;
- restore crashes remain quarantined and non-serving;
- unsaved bytes never enter any revision/object/manifest/lease/restore operation.

## Typed failures and reasons

- `RESIDENCY_DOMAIN_MISMATCH`
- `OBJECT_ADDRESS_INVALID`
- `OBJECT_TOO_LARGE`
- `OBJECT_WRITE_OUTCOME_UNKNOWN`
- `CAS_OBJECT_CORRUPT`
- `CAS_OBJECT_CONFLICT`
- `SOURCE_REVISION_INVALID`
- `SOURCE_REVISION_UNAVAILABLE`
- `SOURCE_OWNER_GENERATION_CHANGED`
- `ANCHOR_MAPPING_FAILED`
- `MATERIALIZATION_LOSS`
- `RETENTION_LEASE_CONFLICT`
- `RETENTION_LEASE_INELIGIBLE`
- `LIFECYCLE_ROOT_INCOMPLETE`
- `OBJECT_INVENTORY_STALE`
- `OBJECT_GRAPH_INVALID`
- `OBJECT_DELETE_NOT_AUTHORIZED`
- `OBJECT_DELETE_OUTCOME_UNKNOWN`
- `OBJECT_DELETE_PARTIAL`
- `PURGE_TOMBSTONE_CONFLICT`
- `RESTORE_PENDING_REVALIDATION`
- `RESTORE_OBJECT_INVALID`
- `RESTORE_ADMISSION_BLOCKED`

## Required tests / qualification evidence

- equal bytes deduplicate only under complete compatible residency closure;
- cross-domain physical dedup/address collision denied;
- temporary write/fsync/atomic rename/reopen and every crash boundary;
- timeout after atomic rename reconstructs success or conflict by exact readback;
- `A → B → A` occurrence revisions remain distinct;
- exact revision readback never substitutes current path/newer revision;
- UTF-8/CRLF/UTF-16/lossy coordinate and anchor fixtures;
- durable lease purpose/owner/expiry/idempotency and unsaved rejection;
- lease release preserves objects until mark/sweep proof;
- root/inventory pagination stays one generation and reports corruption/gaps;
- manifest graph cycles/depth/schema/residency violations fail closed;
- exact deletion only with accepted sweep/purge authority and exact addresses;
- delete timeout applied/none/partial/conflict readback matrix;
- broad/prefix/digest-only deletion path absent;
- tombstone blocks write/import/reindex/restore resurrection;
- restore remains quarantined across restart and until exact admission receipts;
- paired manifest/object/residency/purge mismatch rejected;
- ordinary sweep/security purge receipts remain distinct and never claim secure erase;
- fake control/lifecycle/source ports prove no policy/retention ownership leakage;
- public/default diagnostics contain no source bodies, secrets or unrestricted paths.
