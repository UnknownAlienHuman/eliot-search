# Function contract — `search-research-export-adapter`

**Status:** optional disabled-by-default P14 `eliotr.normalized.v1` export profile; no implementation
exists yet.

The adapter exports a verified immutable Search materialization. It does not perform online research,
transfer ownership through ordinary export or share Search storage/credentials.

## `validate_profile_activation`

```text
validate_profile_activation(feature, config, generic_edge_receipt, export_fixture_receipt, binding_policy)
  -> Result<ResearchExportProfile, ExportError>
```

Requires compiled feature, explicit enablement, accepted generic edge, exact manifest/export fixture and
binding policy. Protocol spelling and canonical manifest-body digest are locked.

Failure leaves the optional profile disabled and generic Search unaffected.

## `prepare_export`

```text
prepare_export(request, source_snapshot, materialization, live_state, limits)
  -> Result<ExportPlan, ExportError>
```

Requires exact immutable retained source revision/materialization, owner generation, source/workspace
view, access/disclosure/allowed-use/residency/retention/purge authorization, purpose, expiry and bounded
content set.

Unsaved buffer/overlay targets are rejected unless a separate explicit snapshot admission already
created a durable `SourceRevision` and residency receipt. Current-path bytes cannot substitute the
planned revision.

Plan contains exact logical relative paths and expected native identities. It owns no container bytes
yet.

## `reopen_and_verify_native_content`

```text
reopen_and_verify_native_content(plan, revision_port, materialization_port, context)
  -> Result<VerifiedExportContent, ExportError>
```

Reopens the exact retained revision/materialization and verifies native BLAKE3 identities, byte lengths,
coordinate/loss maps, profile/config identity and live authorization. Qdrant payload text and cached
snippets are invalid sources.

Cancellation/timeout/unreadable/mismatch returns no partial content eligible for bundle publication.

## `compute_wire_digests`

```text
compute_wire_digests(content, budget, cancel) -> Result<WireDigestSet, ExportError>
```

Independently computes protocol-required SHA-256 over exact logical file bytes. It never relabels an
internal BLAKE3 digest. Equal bytes produce equal wire digests; source bytes are not logged.

## `build_manifest`

```text
build_manifest(plan, verified_content, wire_digests, ownership_validation)
  -> Result<NormalizedBundleManifest, ExportError>
```

Emits exactly `eliotr.normalized.v1`, rejects unknown load-bearing fields and verifies the canonical
manifest-body schema digest:

```text
3a5f9fd2b254eebe574b2c4a28f9804df0da9df359e59ceee125fa7da90fef22
```

Optional fields appear only under protocol rules. Manifest records native provenance, exact source/view
fence, residency/disclosure/allowed-use/expiry, normalization identity, wire digests, capabilities and
quality/assurance warnings.

## `validate_ownership_mode`

```text
validate_ownership_mode(mode, source_owner, cutover_receipt, identity_mapping)
  -> Result<OwnershipModeValidation, ExportError>
```

Rules:

- `federated_reference`: no cutover receipt field; Search ownership unchanged;
- `immutable_import`: no cutover receipt field; output is an immutable import candidate;
- `ownership_cutover`: requires a completed verified source-owner cutover receipt binding old owner
  generation/source-view fence, identity mapping, new owner generation and activation.

The export manifest can record a completed cutover but cannot authorize/perform one. Receipt presence in
other modes is an error.

## `validate_bundle_paths`

```text
validate_bundle_paths(entries, limits) -> Result<CanonicalBundleLayout, ExportError>
```

Allows only registered normalized relative paths. Rejects absolute/rooted paths, `..`, empty/dot/device
components, alternate separators where unsafe, duplicate normalized/case-folded paths, symlinks,
hardlinks, reparse escapes and entry/count/byte/depth overflow.

## `assemble_bundle`

```text
assemble_bundle(layout, manifest, content, sink, operation, context)
  -> Result<ExportReceipt, ExportError>
```

Writes to an isolated temporary destination through a replaceable bounded sink, verifies each written
file SHA-256/length and publishes only after the complete manifest/content set and live authorization
recheck pass.

Container format is a qualified leaf dependency; logical bytes/paths/digests are canonical. No
cross-residency-domain physical dedup or encryption-key reuse.

## `recover_export_operation`

```text
recover_export_operation(operation_id, plan_digest, sink_state, live_state)
  -> Result<ExportRecoveryDecision, ExportError>
```

Same operation ID plus same plan can reconstruct complete receipt, resume a bounded temporary write or
clean an unpublished partial artifact. Same ID plus different plan is rejected.

Timeout/cancel after temporary write is not published success. If final publication may have committed,
readback verifies exact manifest/content digests before reconstructing receipt. Revoked/purged/expired
state prevents publication and triggers cleanup/quarantine as technically possible.

## `emit_export_receipt`

```text
emit_export_receipt(plan, manifest, content_receipts, publication_receipt)
  -> Result<ExportReceipt, ExportError>
```

Binds source namespace/owner generation/revision/view, materialization profile/config, residency and
disclosure policy, ownership mode/validated cutover receipt, logical path/digest set, purpose/expiry,
container identity and limitations.

It claims no online research conclusion, no source ownership transfer except recording an already
completed cutover, and no physical secure erase.

## Typed failures

- `EXPORT_PROFILE_DISABLED`
- `EXPORT_PROFILE_NOT_QUALIFIED`
- `EXPORT_SOURCE_UNAVAILABLE`
- `EXPORT_SOURCE_FENCE_CHANGED`
- `EXPORT_DIGEST_MISMATCH`
- `EXPORT_MANIFEST_SCHEMA_MISMATCH`
- `EXPORT_UNKNOWN_LOAD_BEARING_FIELD`
- `UNSAVED_SNAPSHOT_NOT_ADMITTED`
- `DISCLOSURE_NOT_AUTHORIZED`
- `RESIDENCY_DOMAIN_MISMATCH`
- `OWNERSHIP_CUTOVER_RECEIPT_INVALID`
- `EXPORT_PATH_UNSAFE`
- `EXPORT_LIMIT_EXCEEDED`
- `EXPORT_OPERATION_CONFLICT`
- `EXPORT_OUTCOME_UNKNOWN`
- `EXPORT_PUBLICATION_REVOKED`

## Required tests

- disabled by default and every activation prerequisite;
- exact manifest body/schema digest golden;
- native BLAKE3 verification and independently computed wire SHA-256;
- unknown load-bearing field rejection;
- current-path/Qdrant/cached text cannot substitute exact retained readback;
- unsaved export rejected absent admitted snapshot;
- federated/import modes omit cutover receipt and do not transfer ownership;
- cutover mode exact completed receipt and mapping validation;
- path traversal/absolute/device/duplicate/symlink/hardlink/reparse corpus;
- count/byte/depth bounds and cancellation cleanup;
- timeout before/after final publication readback recovery;
- purge/revocation during export blocks publication;
- cross-residency dedup/key reuse rejected;
- receipt makes no research conclusion or secure-erasure claim.
