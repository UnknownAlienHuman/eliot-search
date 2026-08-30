# Function contract — `search-source-registry`

**Status:** W2/P03 logical contract; no registry persistence, cutover or source-view evidence exists.

This package owns admitted roots, sources, memberships, reference portfolios, coherent source/workspace
views and source-namespace owner transitions. It persists accepted identity/admission decisions through a
vendor-neutral control port and never reimplements those semantics.

## Global rules

- one admitted source namespace has at most one active mutable owner generation;
- old owner is durably fenced before new owner activation;
- root/source/membership/view/portfolio/cutover mutations use stable operation identity and expected
  generation guards;
- membership creation requires an exact current verified admission receipt;
- source/reference scope is explicit and versioned; nearest repository, implicit HEAD or disk-wide scope
  does not exist;
- one compound operation resolves one coherent immutable registry/source/workspace view;
- no source bytes, extracted text, vectors, Qdrant IDs or concrete redb types cross/store here.

## Snapshot operations

### `validate_registry_snapshot(snapshot) -> Result<ValidatedRegistrySnapshot, RegistryError>`

Checks registry/root/source/membership/portfolio/owner generations, uniqueness, referential integrity,
closed states and bounded counts. Missing/duplicate/incoherent records fail closed.

### `snapshot_digest(snapshot) -> RegistrySnapshotDigest`

Domain-separated canonical digest over technical registry state and immutable refs, excluding display
paths/content beyond the accepted disclosure schema.

### `redacted_registry_view(snapshot, binding, disclosure) -> RegistryView`

Returns only authorized root/source/membership/portfolio metadata and reason classes. Foreign membership
names/counts/readiness are not disclosed.

## Root registration

### `register_root(request, identity, admission_policy, control_port, operation, deadline, cancel) -> Result<RootRegistrationReceipt, RegistryError>`

Requires exact current data-root/registry owner fence, resolved local root identity and validated policy
fingerprint/revision. It creates one explicit root record with no implicit recursive source admission.

Same operation plus same request is idempotent. Timeout after possible control commit is unknown until
root/operation readback. Conflicting canonical root or operation input rejects.

### `update_root_policy(root, expected_revision, new_policy, control_port, operation) -> Result<RootPolicyChangeReceipt, RegistryError>`

Commits the new policy fence and emits restrictive/permissive reconciliation/invalidation obligations.
It does not reevaluate/read files or silently alter membership.

### `unbind_root(root, expected_revision, policy, control_port, operation) -> Result<RootUnbindReceipt, RegistryError>`

Fences new admission/ownership under the root and creates explicit source/membership/view invalidation
work. It never deletes revisions/index points or claims purge.

## Source admission and membership

### `admit_source(identity, root, admission_receipt, expected_snapshot, control_port, operation) -> Result<AdmittedSourceReceipt, RegistryError>`

Verifies exact root/policy/observation/decision/source-identity generations and requires `ALLOW` under the
current policy. Stale, review-required, denied or foreign receipt rejects.

The commit creates technical registry state only; it does not read/retain bytes or publish projection.

### `revalidate_admitted_source(source, admission_receipt, expected_revision, control_port, operation) -> Result<SourceAdmissionUpdateReceipt, RegistryError>`

Records the exact current decision/policy fence and required invalidation/reconcile actions. A
restrictive deny fences later preparation/serving through downstream security/currentness owners; it is
not silently delayed until reindex.

### `bind_membership(request, source, admission_receipt, expected_snapshot, control_port, operation) -> Result<SourceMembershipReceipt, RegistryError>`

Requires admitted source, current verified receipt, explicit corpus/membership/role/access/scoring/
residency policies and no duplicate/conflicting membership identity. One source may have multiple
explicit memberships; each projection point later has one projection membership.

### `transition_membership(membership, command, expected_revision, control_port, operation) -> Result<MembershipTransitionReceipt, RegistryError>`

Closed commands activate, restrict, suspend, retire or remove membership with monotonic revision and
explicit downstream security/publication/retention obligations. Removal is not purge or byte deletion.

## Reference portfolios and views

### `publish_reference_portfolio(portfolio, expected_revision, control_port, operation) -> Result<PortfolioReceipt, RegistryError>`

Validates explicit ordered/typed memberships, lineage roles, policy/profile refs and finite bounds. Empty
portfolio returns a typed reason unless the request explicitly permits it.

### `resolve_source_view(request, snapshot, access_snapshot, currentness_snapshot, limits) -> Result<ResolvedSourceView, RegistryError>`

Resolves exact explicit roots/sources/memberships/portfolio/revision policy into one immutable view with:

- registry and owner generations;
- allowed membership identities;
- source/workspace-view revision refs;
- disclosure/residency/profile fences;
- explicit excluded/denied/missing/ambiguous reasons.

It cannot infer nearest repository, broaden to disk or choose implicit current HEAD. Access/currentness
snapshots are consumed as exact accepted inputs; the registry does not decide their meaning.

### `resolve_workspace_view(request, snapshot, workspace_observations, limits) -> Result<WorkspaceViewResolution, RegistryError>`

Binds one coherent repository/worktree/branch/index/buffer fence for compound operations. Branch/index/
buffer-revision changes yield a new workspace-view revision. Nested repositories/submodules remain
explicit boundaries.

### `verify_view(view, current_snapshot, current_owner) -> Result<VerifiedRegistryView, RegistryError>`

Rechecks registry/root/membership/portfolio/owner/view generations before a downstream operation uses the
view. Stale or mixed-generation view fails rather than silently replans under the same operation.

## Namespace ownership and cutover

### `prepare_namespace_cutover(namespace, current_owner, proposed_owner, policy, control_port, operation) -> Result<CutoverPreparation, RegistryError>`

Validates exact current owner generation, new owner identity/incarnation, affected sources/memberships and
migration/export evidence. It creates a durable preparation record but does not activate the new owner.

### `fence_old_owner(preparation, control_port, operation) -> Result<OwnerFenceReceipt, RegistryError>`

Durably prevents new mutable source/revision publication by the old owner generation before any new owner
activation. The fence is monotonic and immediately visible to view verification.

### `activate_new_owner(preparation, fence, import_or_transfer_receipt, control_port, operation) -> Result<SourceOwnerActivationReceipt, RegistryError>`

Requires old-owner fence, exact accepted transfer/import state and no conflicting owner. It advances
`SourceOwnerGeneration`; old owner can never resume under the prior generation.

### `verify_cutover_receipt(receipt, old_state, new_state, snapshot) -> Result<VerifiedCutoverReceipt, RegistryError>`

Proves fence-before-activation ordering, namespace/source/membership coverage and owner-generation
advance. Ordinary export/copy is not accepted as ownership cutover.

### `recover_cutover(operation, control_port) -> Result<CutoverRecoveryDecision, RegistryError>`

Returns prepared, old-owner-fenced, new-owner-active, completed, conflicting or quarantined state from
durable records. It never activates based on process presence or partial external copy alone.

## Mutation recovery and batch operations

### `recover_registry_mutation(operation, expected_digest, control_port) -> Result<RegistryMutationRecovery, RegistryError>`

Resolves unknown commit outcome by exact idempotency/entity/generation readback. It reconstructs success,
permits same-operation retry only when not committed, or fails/quarantines on conflict/partial state.

### `apply_admission_batch(batch, expected_snapshot, control_port, limits, deadline, cancel) -> Result<RegistryBatchReceipt, RegistryError>`

Processes finite canonical items and commits one outcome per source. Cancellation before commit is clean;
after possible commit, recovery uses the batch operation identity. Partial per-item business outcomes are
explicit and never collapsed to total success.

## Cancellation, deadline and crash semantics

Pure validation/view operations are retry-safe. All durable mutations have finite deadlines, stable
operation identity, expected generation guards and exact readback recovery. Cancellation after possible
commit is unknown until recovered. No retry widens scope, changes owner or substitutes a newer snapshot
under the old operation.

## Typed failures

- `REGISTRY_SNAPSHOT_INVALID`
- `ROOT_IDENTITY_CONFLICT`
- `ROOT_ALREADY_REGISTERED`
- `ROOT_NOT_REGISTERED`
- `ROOT_POLICY_GENERATION_MISMATCH`
- `SOURCE_NOT_ADMITTED`
- `SOURCE_ALREADY_ADMITTED_CONFLICT`
- `ADMISSION_RECEIPT_STALE`
- `ADMISSION_RECEIPT_MISMATCH`
- `MEMBERSHIP_CONFLICT`
- `MEMBERSHIP_GENERATION_MISMATCH`
- `REFERENCE_SCOPE_EMPTY`
- `REFERENCE_PORTFOLIO_INVALID`
- `SOURCE_VIEW_AMBIGUOUS`
- `SOURCE_VIEW_STALE`
- `WORKSPACE_VIEW_INCOHERENT`
- `SOURCE_NAMESPACE_OWNERSHIP_CONFLICT`
- `SOURCE_OWNER_CUTOVER_REQUIRED`
- `CUTOVER_RECEIPT_MISMATCH`
- `REGISTRY_MUTATION_OUTCOME_UNKNOWN`
- `REGISTRY_OPERATION_CONFLICT`
- `REGISTRY_CANCELLED_BEFORE_COMMIT`

## Required tests / qualification evidence

- root register/replay/conflict and policy-change obligations;
- membership requires exact current allow receipt and cannot weaken admission;
- restrictive revalidation immediately fences downstream view use;
- explicit portfolio/source view; no nearest-repo/HEAD/disk-wide fallback;
- one coherent registry/source/workspace generation across compound operation;
- nested repository/submodule/worktree/branch/index fixtures;
- unauthorized/foreign membership name/count/readiness non-disclosure;
- dual active mutable owner impossible;
- cutover old-owner fence durably precedes new activation;
- owner generation advances and old owner cannot resume;
- ordinary export/copy cannot satisfy cutover receipt;
- crash/unknown outcome at every root/source/membership/portfolio/cutover transaction;
- batch accounts one outcome per input and cancellation recovery;
- large bytes/point lists absent; only immutable manifest refs allowed;
- no concrete redb/filesystem/Git/Qdrant dependency or source content;
- vendor-neutral control-port fakes and canonical snapshot/view/receipt goldens.
