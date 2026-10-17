//! One complete pure admission before either submit or restore hydrates state.

use search_contracts::{self as c, CanonicalValue as V, indexed::CollectionSchema};
use search_projection_planner::{
    ManifestDiff, PlanningScope, ProjectionBudget, ProjectionManifest, ProjectionPlan,
    canonical_projection_manifest_semantic_digest, diff_manifests, manifest_canonical_bytes,
    validate_schema, validate_scope_binding, verified_projection_manifest_semantic_digest,
};
use search_unitizer::VerifiedUnitSet;

use crate::{PublicationError as Error, PublicationGuards, model::PreparedGeneration};

/// Explicit source and index coordinates; none is inferred from an empty list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationScope {
    /// Installation of the qualified physical route.
    pub installation_incarnation_id: c::InstallationIncarnationId,
    /// Exact collection generation.
    pub collection_generation_id: c::CollectionGenerationId,
    /// Exact projection and source memberships.
    pub projection_membership_id: c::ProjectionMembershipId,
    /// Registry source membership, not a corpus or source alias.
    pub source_membership_id: c::SourceMembershipId,
    /// Full retained revision identity and direct byte commitment.
    pub source: c::SourceRevisionRef,
    /// Actual admitted materialization identity.
    pub materialization_id: c::MaterializationId,
    /// Actual admitted representation identity.
    pub representation_id: c::RepresentationId,
    /// Owning complete `UnitSet` commitment.
    pub unit_manifest_digest: c::Blake3Digest32,
    /// Complete accepted projection profile set.
    pub projection_profile_set_id: c::ProjectionProfileSetId,
    /// Logical projection schema and full physical schema commitment.
    pub projection_schema_id: c::ProfileId,
    /// C00-owned schema identity, independent of its logical name.
    pub schema_identity: c::Blake3Digest32,
    /// Actual pre-scoring access population.
    pub access_partition_id: c::AccessPartitionId,
    /// Actual scoring/IDF population.
    pub scoring_partition_id: c::ScoringPartitionId,
}

/// Explicit publication target; retirement never impersonates a new empty set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublicationTarget {
    /// A complete newly qualified representation.
    Representation(PublicationScope),
    /// A zero-entry publication target, retaining its exact prior source scope.
    Retirement(PublicationScope),
}
impl PublicationTarget {
    /// Source/index coordinates; these are prior coordinates for retirement.
    #[must_use]
    pub const fn scope(&self) -> &PublicationScope {
        match self {
            Self::Representation(scope) | Self::Retirement(scope) => scope,
        }
    }
}

/// Read-only owner products, reachable only through the trusted preparation port.
pub struct ProjectionEvidence<'a> {
    /// Exact retained revision, not a current filesystem path.
    pub source: &'a c::SourceRevisionRef,
    /// Owning opaque complete set.
    pub units: &'a VerifiedUnitSet,
    /// Owner-reconstructable point plan and canonical manifest.
    pub plan: &'a ProjectionPlan,
    /// Original complete point scope and validity.
    pub planning_scope: &'a PlanningScope,
    /// Qualified route's exact schema data.
    pub schema: &'a CollectionSchema,
}

/// Preparation and original restoration share the same static validator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionPurpose {
    /// Propose exactly the next unused epoch, without reserving it.
    Prepare,
    /// Verify the original retained epoch; never allocate another one.
    Restore,
}
/// Actual membership state supplied by the current/historical owning read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationMembershipState {
    /// New points may be proposed after owner verification.
    Active,
    /// Only complete retirement, with no new representation, is eligible.
    Retired,
}
/// Original-context work; this is not an independent meter or a new deadline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionWork {
    /// Poll the actual original cancellation/deadline and native owner.
    Checkpoint,
    /// Reserve bounded logical copies before construction.
    Allocate(usize),
    /// Reserve bounded processing over immutable metadata.
    Process(usize),
}

/// Control observations from one actual owner token, never request claims.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicationOwnerObservation {
    /// Original guards captured with this preparation/retained intent.
    pub original_guards: PublicationGuards,
    /// Current guards; restore uses a changed value only for compensation.
    pub current_guards: PublicationGuards,
    /// Original visible predecessor of this reservation.
    pub previous_visible_epoch: c::Epoch,
    /// Current durable visibility.
    pub current_visible_epoch: c::Epoch,
    /// Actual consumed reservation floor from durable history.
    pub last_reserved_epoch: c::Epoch,
    /// Next proposal for Prepare, exact retained target for Restore.
    pub target_epoch: c::Epoch,
    /// Registry-owned lifecycle, not an empty-list assertion.
    pub membership_state: PublicationMembershipState,
}

/// Trusted producer seam over the real preparation and control/source owners.
///
/// Implementations must borrow their existing private preparation and actual
/// journal/registry/native snapshot tokens, hold the original owner lock, and
/// recheck live owner/profile/source/namespace state. No default accepts a
/// caller digest, receipt, complete flag or copy of the proposed fields.
pub trait PublicationAdmissionOwners {
    /// Existing private C08/C09/C13 prepared product (or qualified rebuild).
    type Projection: ?Sized;
    /// Existing private coherent control/source owner observation.
    type Snapshot: ?Sized;
    /// Borrow complete immutable evidence from the exact producer token.
    fn projection<'a>(&self, token: &'a Self::Projection) -> Result<ProjectionEvidence<'a>, Error>;
    /// Independently verify this complete scope against actual registry,
    /// source/namespace, profile and route owners and the original operation.
    fn observe(
        &mut self,
        snapshot: &Self::Snapshot,
        transaction: &c::OpaqueId,
        scope: &PublicationScope,
        purpose: AdmissionPurpose,
    ) -> Result<PublicationOwnerObservation, Error>;
    /// Reopen/verify the actual C07 manifest named by the current C02 pointer
    /// and compare its complete canonical content. Absence must be observed.
    fn verify_current_manifest(
        &mut self,
        snapshot: &Self::Snapshot,
        scope: &PublicationScope,
        expected: Option<&ProjectionManifest>,
    ) -> Result<(), Error>;
    /// All validation borrows the caller's original allowance.
    fn work(&mut self, snapshot: &Self::Snapshot, work: AdmissionWork) -> Result<(), Error>;
}

/// Ordinary requested change; selecting Retirement is not proof of retirement.
pub enum ProjectionUpdate<'a, T: ?Sized> {
    /// Publish the complete qualified representation.
    Publish(&'a T),
    /// Retire the complete old set after an actual retired-owner observation.
    Retirement,
}
/// Requested inputs contain no caller-computed digest or preparation receipt.
pub struct PublicationProposal<'a, T: ?Sized> {
    /// Full original operation identity, retained unchanged across recovery.
    pub transaction_id: c::OpaqueId,
    /// Actual old preparation token, absent only after observed pointer absence.
    pub old: Option<&'a T>,
    /// Qualified new preparation or explicit whole-membership retirement.
    pub update: ProjectionUpdate<'a, T>,
}

fn scope(
    evidence: &ProjectionEvidence<'_>,
    budget: ProjectionBudget,
) -> Result<PublicationScope, Error> {
    validate_scope_binding(evidence.units, evidence.planning_scope, budget)
        .map_err(|_| Error::InvalidPreparedPublication)?;
    validate_schema(evidence.schema, evidence.plan, budget)
        .map_err(|_| Error::InvalidPreparedPublication)?;
    let provenance = evidence.units.manifest().provenance();
    let binding = provenance.binding();
    let membership = &evidence.planning_scope.membership;
    if evidence.source.source_namespace_id != binding.source_namespace_id
        || evidence.source.source_id != binding.source_id
        || evidence.source.revision_id != binding.source_revision_id
        || evidence.source.content_digest != provenance.input_digest()
        || evidence.source.byte_length != provenance.native_bytes()
        || evidence.plan.source_membership_id != membership.source_membership_id
        || evidence.plan.projection_membership_id != membership.projection_membership_id
    {
        return Err(Error::InvalidPreparedPublication);
    }
    let units = evidence
        .units
        .units()
        .iter()
        .map(|unit| (unit.unit_id(), unit))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut seen = std::collections::BTreeSet::new();
    if evidence.plan.manifest.entries.len() != units.len() {
        return Err(Error::InvalidPreparedPublication);
    }
    for entry in &evidence.plan.manifest.entries {
        let unit = units
            .get(&entry.unit_id)
            .ok_or(Error::InvalidPreparedPublication)?;
        if !seen.insert(entry.unit_id)
            || entry.unit_digest != unit.unit_content_digest()
            || entry.reference_digest != unit.reference_digest()
        {
            return Err(Error::InvalidPreparedPublication);
        }
    }
    Ok(PublicationScope {
        installation_incarnation_id: membership.installation_incarnation_id,
        collection_generation_id: membership.collection_generation_id,
        projection_membership_id: membership.projection_membership_id,
        source_membership_id: membership.source_membership_id,
        source: *evidence.source,
        materialization_id: binding.materialization_id,
        representation_id: binding.representation_id,
        unit_manifest_digest: evidence.units.manifest_digest(),
        projection_profile_set_id: membership.projection_profile_set_id.clone(),
        projection_schema_id: membership.projection_schema_id.clone(),
        schema_identity: *evidence
            .schema
            .identity()
            .map_err(|_| Error::InvalidPreparedPublication)?
            .as_digest(),
        access_partition_id: membership.access_partition_id,
        scoring_partition_id: membership.scoring_partition_id,
    })
}

fn same_binding(a: &PublicationScope, b: &PublicationScope) -> bool {
    a.installation_incarnation_id == b.installation_incarnation_id
        && a.collection_generation_id == b.collection_generation_id
        && a.source_membership_id == b.source_membership_id
        && a.projection_membership_id == b.projection_membership_id
        && a.source.source_namespace_id == b.source.source_namespace_id
        && a.source.source_id == b.source.source_id
        && a.projection_profile_set_id == b.projection_profile_set_id
        && a.schema_identity == b.schema_identity
        && a.projection_schema_id == b.projection_schema_id
}

fn checked_scope<P: PublicationAdmissionOwners + ?Sized>(
    owners: &mut P,
    snapshot: &P::Snapshot,
    evidence: &ProjectionEvidence<'_>,
    budget: ProjectionBudget,
) -> Result<(PublicationScope, c::Blake3Digest32), Error> {
    owners.work(snapshot, AdmissionWork::Checkpoint)?;
    if evidence.units.units().len() > budget.max_points
        || evidence.plan.points.len() > budget.max_points
        || evidence.plan.manifest.entries.len() > budget.max_points
        || evidence.plan.manifest.canonical_bytes.len() > budget.max_manifest_bytes
    {
        return Err(Error::BudgetExceeded);
    }
    let payload = evidence.plan.points.iter().try_fold(0_usize, |n, point| {
        n.checked_add(point.expected_readback.payload_cbor.len())
            .ok_or(Error::BudgetExceeded)
    })?;
    let allocation = payload
        .checked_add(evidence.plan.manifest.canonical_bytes.len())
        .and_then(|n| n.checked_mul(16))
        .and_then(|n| {
            evidence
                .units
                .units()
                .len()
                .checked_mul(8192)
                .and_then(|units| n.checked_add(units))
        })
        .and_then(|n| n.checked_add(32768))
        .ok_or(Error::BudgetExceeded)?;
    owners.work(snapshot, AdmissionWork::Allocate(allocation))?;
    owners.work(snapshot, AdmissionWork::Process(allocation))?;
    let result = scope(evidence, budget)?;
    let digest = verified_projection_manifest_semantic_digest(
        &evidence.plan.manifest,
        &evidence.plan.points,
        evidence.planning_scope,
        budget,
    )
    .map_err(|_| Error::InvalidPreparedPublication)?;
    owners.work(snapshot, AdmissionWork::Checkpoint)?;
    Ok((result, digest))
}

fn validate_epoch_and_lifecycle(
    observed: PublicationOwnerObservation,
    purpose: AdmissionPurpose,
    old: Option<&ProjectionEvidence<'_>>,
    new: Option<&ProjectionEvidence<'_>>,
) -> Result<(), Error> {
    let target = observed.target_epoch;
    if target <= observed.previous_visible_epoch
        || observed.last_reserved_epoch < observed.previous_visible_epoch
    {
        return Err(Error::EpochMismatch);
    }
    match purpose {
        AdmissionPurpose::Prepare
            if observed.last_reserved_epoch.checked_next().ok() != Some(target)
                || observed.current_visible_epoch != observed.previous_visible_epoch
                || observed.original_guards != observed.current_guards =>
        {
            return Err(Error::GuardMismatch);
        }
        AdmissionPurpose::Restore
            if observed.last_reserved_epoch != target
                || (observed.current_visible_epoch != observed.previous_visible_epoch
                    && observed.current_visible_epoch != target) =>
        {
            return Err(Error::EpochMismatch);
        }
        _ => {}
    }
    if (new.is_some() && observed.membership_state != PublicationMembershipState::Active)
        || (new.is_none() && observed.membership_state != PublicationMembershipState::Retired)
        || old.as_ref().is_some_and(|e| {
            e.planning_scope.valid_from_epoch > observed.previous_visible_epoch
                || e.planning_scope
                    .valid_until_epoch_exclusive
                    .is_some_and(|until| until <= observed.previous_visible_epoch)
        })
    {
        return Err(Error::InvalidPreparedPublication);
    }
    Ok(())
}

fn validate_new_validity(
    new: Option<&ProjectionEvidence<'_>>,
    old: Option<&ProjectionEvidence<'_>>,
    difference: &ManifestDiff,
    observed: PublicationOwnerObservation,
) -> Result<(), Error> {
    if let Some(new) = new {
        let scope = new.planning_scope;
        if difference.create.is_empty() {
            if scope.valid_from_epoch > observed.previous_visible_epoch {
                return Err(Error::EpochMismatch);
            }
        } else if scope.valid_from_epoch != observed.target_epoch
            || scope.valid_until_epoch_exclusive.is_some()
        {
            return Err(Error::EpochMismatch);
        }
        if let Some(old) = old {
            let old_points = old
                .plan
                .points
                .iter()
                .map(|point| (point.point_id, point))
                .collect::<std::collections::BTreeMap<_, _>>();
            let new_points = new
                .plan
                .points
                .iter()
                .map(|point| (point.point_id, point))
                .collect::<std::collections::BTreeMap<_, _>>();
            for retained in &difference.retain {
                if old_points.get(&retained.point_id) != new_points.get(&retained.point_id) {
                    return Err(Error::InvalidPreparedPublication);
                }
            }
        }
    }
    Ok(())
}

fn canonical_empty_manifest(budget: ProjectionBudget) -> Result<ProjectionManifest, Error> {
    let mut manifest = ProjectionManifest {
        entries: Vec::new(),
        canonical_bytes: Vec::new(),
    };
    manifest.canonical_bytes = manifest_canonical_bytes(&manifest, budget)
        .map_err(|_| Error::InvalidPreparedPublication)?
        .as_slice()
        .to_vec();
    Ok(manifest)
}

/// Complete canonical admission shared by normal submit and pre-hydration restore.
/// No active slot, epoch, journal or index is changed by this function.
pub fn validate_prepared_generation<P: PublicationAdmissionOwners + ?Sized>(
    owners: &mut P,
    snapshot: &P::Snapshot,
    proposal: &PublicationProposal<'_, P::Projection>,
    purpose: AdmissionPurpose,
    budget: ProjectionBudget,
) -> Result<PreparedGeneration, Error> {
    let budget = budget.validate().map_err(|_| Error::BudgetExceeded)?;
    owners.work(snapshot, AdmissionWork::Checkpoint)?;
    let old = proposal
        .old
        .map(|token| owners.projection(token))
        .transpose()?;
    let new = match &proposal.update {
        ProjectionUpdate::Publish(token) => Some(owners.projection(token)?),
        ProjectionUpdate::Retirement => None,
    };
    let (old_scope, old_digest) = old
        .as_ref()
        .map(|e| checked_scope(owners, snapshot, e, budget))
        .transpose()?
        .unzip();
    let (new_scope, new_digest) = new
        .as_ref()
        .map(|e| checked_scope(owners, snapshot, e, budget))
        .transpose()?
        .unzip();
    let admitted_scope = new_scope
        .as_ref()
        .or(old_scope.as_ref())
        .ok_or(Error::InvalidPreparedPublication)?;
    if old_scope
        .as_ref()
        .is_some_and(|old| !same_binding(old, admitted_scope))
    {
        return Err(Error::InvalidPreparedPublication);
    }
    let observed = owners.observe(snapshot, &proposal.transaction_id, admitted_scope, purpose)?;
    let target = observed.target_epoch;
    validate_epoch_and_lifecycle(observed, purpose, old.as_ref(), new.as_ref())?;
    let old_manifest = old.as_ref().map(|e| &e.plan.manifest);
    let empty = canonical_empty_manifest(budget)?;
    let new_manifest = new.as_ref().map_or(&empty, |e| &e.plan.manifest);
    let current =
        if purpose == AdmissionPurpose::Restore && observed.current_visible_epoch == target {
            new.as_ref().map(|e| &e.plan.manifest)
        } else {
            old_manifest
        };
    owners.verify_current_manifest(snapshot, admitted_scope, current)?;
    let empty_old = &empty;
    let difference = diff_manifests(old_manifest.unwrap_or(empty_old), new_manifest)
        .map_err(|_| Error::InvalidPreparedPublication)?;
    validate_new_validity(new.as_ref(), old.as_ref(), &difference, observed)?;
    if crate::machine::changed_point_id_is_reused(old_manifest, new_manifest) {
        return Err(Error::InvalidPreparedPublication);
    }
    let new_digest = match new_digest {
        Some(digest) => digest,
        None => canonical_projection_manifest_semantic_digest(&empty, budget)
            .map_err(|_| Error::InvalidPreparedPublication)?,
    };
    let prepared_input_digest = prepared_digest(PreparedDigestInput {
        transaction: &proposal.transaction_id,
        old_scope: old_scope.as_ref(),
        scope: admitted_scope,
        target,
        previous: observed.previous_visible_epoch,
        guards: observed.original_guards,
        old: old_manifest,
        new: new_manifest,
        difference: &difference,
        retirement: new.is_none(),
    })?;
    owners.work(snapshot, AdmissionWork::Checkpoint)?;
    if owners.observe(snapshot, &proposal.transaction_id, admitted_scope, purpose)? != observed {
        return Err(Error::GuardMismatch);
    }
    Ok(PreparedGeneration {
        transaction_id: proposal.transaction_id.clone(),
        target: if new.is_some() {
            PublicationTarget::Representation(admitted_scope.clone())
        } else {
            PublicationTarget::Retirement(admitted_scope.clone())
        },
        collection_generation_id: admitted_scope.collection_generation_id,
        old_manifest: old_manifest.cloned(),
        new_manifest: new_manifest.clone(),
        old_manifest_digest: old_digest,
        new_manifest_digest: new_digest,
        guards: observed.original_guards,
        prepared_input_digest,
        difference,
        observed,
    })
}

#[derive(Clone, Copy)]
struct PreparedDigestInput<'a> {
    transaction: &'a c::OpaqueId,
    old_scope: Option<&'a PublicationScope>,
    scope: &'a PublicationScope,
    target: c::Epoch,
    previous: c::Epoch,
    guards: PublicationGuards,
    old: Option<&'a ProjectionManifest>,
    new: &'a ProjectionManifest,
    difference: &'a ManifestDiff,
    retirement: bool,
}

fn prepared_digest(input: PreparedDigestInput<'_>) -> Result<c::Blake3Digest32, Error> {
    let PreparedDigestInput {
        transaction,
        old_scope,
        scope,
        target,
        previous,
        guards,
        old,
        new,
        difference,
        retirement,
    } = input;
    let bytes = |v: &[u8]| {
        c::BoundedBytes::new(v.to_vec())
            .map(V::Bytes)
            .map_err(|_| Error::BudgetExceeded)
    };
    let text = |v: &str| {
        c::CanonicalText::new(v)
            .map(V::Text)
            .map_err(|_| Error::BudgetExceeded)
    };
    let array = |v| {
        c::BoundedList::new(v)
            .map(V::Array)
            .map_err(|_| Error::BudgetExceeded)
    };
    let scope_value = |s: &PublicationScope| -> Result<V, Error> {
        array(vec![
            bytes(s.installation_incarnation_id.as_bytes())?,
            bytes(s.collection_generation_id.as_bytes())?,
            bytes(s.projection_membership_id.as_bytes())?,
            bytes(s.source_membership_id.as_bytes())?,
            bytes(s.source.source_namespace_id.as_bytes())?,
            bytes(s.source.source_id.as_bytes())?,
            bytes(s.source.revision_id.as_bytes())?,
            bytes(s.source.content_digest.as_bytes())?,
            V::U64(s.source.byte_length),
            bytes(s.materialization_id.as_bytes())?,
            bytes(s.representation_id.as_bytes())?,
            bytes(s.unit_manifest_digest.as_bytes())?,
            text(s.projection_profile_set_id.as_str())?,
            text(s.projection_schema_id.as_str())?,
            bytes(s.schema_identity.as_bytes())?,
            bytes(s.access_partition_id.as_bytes())?,
            bytes(s.scoring_partition_id.as_bytes())?,
        ])
    };
    let ids = |entries: &[search_projection_planner::ProjectionManifestEntry]| -> Result<V, Error> {
        array(
            entries
                .iter()
                .map(|e| bytes(e.point_id.as_bytes()))
                .collect::<Result<Vec<_>, _>>()?,
        )
    };
    let value = array(vec![
        V::U64(1),
        V::Bool(retirement),
        text(transaction.as_str())?,
        old_scope.map(scope_value).transpose()?.unwrap_or(V::Null),
        scope_value(scope)?,
        V::U64(u64::try_from(target.get()).map_err(|_| Error::EpochMismatch)?),
        V::U64(u64::try_from(previous.get()).map_err(|_| Error::EpochMismatch)?),
        array(vec![
            V::U64(guards.owner_epoch.get()),
            V::U64(guards.source_catalog_generation),
            V::U64(guards.membership_generation),
            V::U64(guards.access_generation),
            V::U64(guards.shadow_generation),
            V::U64(guards.purge_generation),
            bytes(guards.profile_digest.as_bytes())?,
        ])?,
        old.map(|m| bytes(&m.canonical_bytes))
            .transpose()?
            .unwrap_or(V::Null),
        bytes(&new.canonical_bytes)?,
        ids(&difference.create)?,
        ids(&difference.retain)?,
        ids(&difference.retire)?,
    ])?;
    c::blake3_canonical(
        &c::CanonicalDigestDomain::parse("eliot/cbor/prepared-generation/v1")
            .map_err(|_| Error::InvalidPreparedPublication)?,
        &value,
        c::DigestInputLimit::new(c::MAX_CANONICAL_BYTES).map_err(|_| Error::BudgetExceeded)?,
    )
    .map_err(|_| Error::BudgetExceeded)
}
