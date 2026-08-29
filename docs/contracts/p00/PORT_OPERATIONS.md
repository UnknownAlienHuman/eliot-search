# Vendor-neutral port operation inventory

This file defines behavior, not exact Rust async syntax. Every potentially blocking operation accepts
an `OperationContext` with request identity, deadline, cancellation and bounded budget.

Every mutation additionally accepts `MutationIdentity` with an explicit idempotency class. Every
operation returns a typed result/receipt; vendor strings never cross the boundary.

## Runtime and control

### `ClockPort`

- `utc_now() -> UtcTimestamp`
- `monotonic_now() -> MonotonicInstant`
- no wall-clock use for ordering when monotonic time is required

### `SecretStorePort`

- `create_secret(scope, purpose, mutation) -> SecretRef`
- `lease_secret(secret_ref, purpose, context) -> SecretLease`
- `rotate_secret(secret_ref, mutation) -> RotationReceipt`
- `delete_secret(secret_ref, mutation) -> DeletionReceipt`

Plaintext is available only inside a bounded purpose/incarnation-bound lease and is never serializable.

### `ProcessSupervisorPort`

- `qualify_artifact(candidate, context) -> QualifiedArtifact`
- `start_process(owner_fence, artifact, secret_lease, mutation) -> ProcessGuard`
- `verify_process_identity(guard, context) -> ProcessIdentityReceipt`
- `readiness(guard, context) -> ProcessReadiness`
- `shutdown_process(guard, mode, mutation) -> ShutdownReceipt`

### `ControlJournalPort`

- `read_control_snapshot(context) -> ControlSnapshot`
- `transact(command, mutation) -> ControlCommit`
- `compare_and_swap_visible_epoch(guards, commit, mutation) -> ControlCommit`
- `load_unresolved_publication(context) -> PublicationIntent | null`
- `quarantine(reason, mutation) -> QuarantineReceipt`
- `write_counters(context) -> JournalWriteCounters`

### `ControlSnapshotPort`

- `current_snapshot() -> immutable ControlSnapshot`
- reads are process-local and perform no durable write

## Source and residency

### `SourceAdmissionPort`

- `normalize_policy(policy) -> CanonicalAdmissionPolicy`
- `evaluate(policy, observation) -> AdmissionDecision`
- `verify_receipt(receipt, policy, observation) -> unit`

### `SourceInventoryPort`

- `resolve_source_view(request, context) -> ResolvedSourceView`
- `resolve_workspace_view(workspace, context) -> WorkspaceViewRevision`
- `list_exact_denominator(scope, context) -> BoundedPage<SourceRevisionRef>`
- `lookup_source_head(source_id, context) -> SourceRevisionRef | null`
- `read_inventory_revision(context) -> CatalogRevision`

### `SourceOwnershipPort`

- `read_namespace_owner(namespace, context) -> SourceNamespaceOwnership`
- `transition_namespace_owner(command, mutation) -> SourceNamespaceOwnership`
- `verify_cutover_receipt(receipt, context) -> CutoverVerificationReceipt`

### `SafeReaderPort`

- `resolve_final_source(locator, admitted_root, context) -> ResolvedSource`
- `stable_read(resolved, context) -> StableRead`
- `read_git_object_no_execute(repo, oid, path, context) -> StableRead`

### `SourceRevisionStorePort`

- `admit_revision(stable_read, residency, mutation) -> RevisionReceipt`
- `reopen_exact(revision_ref, context) -> VerifiedRevision`
- `retain(revision_ref, cause, mutation) -> RetentionLease`
- `release_retention(lease, mutation) -> ReleaseReceipt`
- `enumerate_mark_roots(snapshot, context) -> BoundedPage<MarkRoot>`

### `ResidencyPolicyPort`

- `resolve_residency(profile_ref, source, context) -> SearchObjectResidencyKey`
- `authorize_copy_or_reencrypt(source_key, target_profile, context) -> ResidencyTransitionPlan`
- `record_transition(receipt, mutation) -> ResidencyTransitionReceipt`

## Preparation and providers

### `MaterializerPort`

- `profile() -> MaterializerProfileDescriptor`
- `materialize(revision, context) -> MaterializationProduct`

### `UnitizerPort`

- `profile() -> UnitizerProfileDescriptor`
- `unitize(materialization, context) -> UnitManifest`

### `CodeEnricherPort`

- `profile() -> EnricherProfileDescriptor`
- `enrich(representation, context) -> StructuralFactSet`

### `LexicalEncoderPort`

- `profile() -> LexicalProfileDescriptor`
- `encode_document(input, context) -> SparseVector`
- `encode_query(input, context) -> SparseVector`
- `fixture_digest() -> Blake3Digest32`

### `ModelProviderPort` — optional

- `profile() -> ModelProfileDescriptor`
- `encode(input, context) -> ModelVectorProduct`
- `rerank(request, context) -> BoundedRerankResult`

No optional provider is called unless its profile is explicitly admitted.

## Index and publication support

### `SearchIndexPort`

- `probe_capabilities(context) -> CapabilityReceipt`
- `ensure_schema(schema, mutation) -> SchemaReceipt`
- `upsert_exact(batch, write_policy, mutation) -> MutationReceipt`
- `close_exact(ids, epoch, write_policy, mutation) -> MutationReceipt`
- `readback_exact(ids, context) -> PointReadback`
- `query(leg, context) -> BoundedStream<IndexCandidate>`
- `exact_count(filter, context) -> u64`

### `SearchIndexAdminPort`

- `delete_exact(ids, write_policy, mutation) -> MutationReceipt`
- `validate_route(route, context) -> RouteValidationReceipt`

No broad delete/update operation exists on a correctness path.

### `EpochPinPort`

- `acquire_epoch_pin(route, epoch, owner, context) -> EpochPinGuard`
- `acquire_route_pin(route, owner, context) -> RoutePinGuard`
- `reclamation_watermark(route, context) -> ReclamationWatermark`
- `release_owner(owner) -> ReleaseReceipt`

## Query/security services

### `AccessCompilerPort`

- `validate_grant(claims, binding, live_state, context) -> ValidatedGrant`
- `intersect_scope(request, grant, snapshot, context) -> AuthorizedScope`
- `compile_safe_legs(scope, proofs, context) -> BoundedList<SafeLeg>`
- `revalidate_checkpoint(checkpoint, live_state, context) -> SecurityPermit`

### `OverlayPort`

- `snapshot_overlay(view, grant, context) -> OverlaySnapshot`
- `shadowed_memberships(snapshot) -> BoundedSet<SourceMembershipId>`
- `direct_candidates(snapshot, request, context) -> BoundedStream<OverlayCandidate>`

Unsaved bytes never appear in a durable receipt or serializable port object.

### `ExactScannerPort`

- `compile_exact_scan(request, inventory, context) -> ExactScanPlan`
- `execute_exact_scan(plan, readback, context) -> ExactExecutionReport`

### `HandleStorePort`

- `mint_ephemeral(subject, binding, limits, context) -> SearchSourceHandle`
- `mint_durable(subject, retention_receipt, binding, limits, mutation) -> SearchSourceHandle`
- `expand(handle, request, live_state, context) -> HandleExpansionResult`
- `invalidate(scope, mutation) -> InvalidationReceipt`
- `expire(now, mutation) -> ExpiryReceipt`

## Conformance requirements

Each port publishes a fake/in-memory implementation able to force:

- deadline before start and during operation;
- cancellation before side effects and after an acknowledged external side effect;
- stale generation / compare-and-swap rejection;
- partial bounded stream or receipt;
- dependency unavailable and fail-closed state;
- idempotent retry with the same mutation identity;
- rejection of an unsafe retry with a different identity.
