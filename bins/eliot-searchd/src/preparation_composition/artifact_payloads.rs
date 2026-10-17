//! Actual C08/C09/C13 artifact bodies prepared from one exact retained revision.
//!
//! This composes existing producers; it grants no source/root/key/publication
//! authority. Root must retain those owners and persist the genuine IDs before
//! any artifact can become a committed Qdrant projection.

use search_contracts::{self as c, indexed::CollectionSchema};
use search_control_redb::MigrationBootstrapWork as Work;
use search_lexical::{AcceptedCodeIdentifiersProfile, CodeIdentifierLimits, SparseLimits};
use search_materializer::api::{
    MaterializationBudget, MaterializationProduct, ValidatedMaterializationRequest,
    ValidatedMaterializerProfile, canonicalize_materialization,
};
use search_projection_planner::{
    PlannedUnit, PlanningScope, ProjectionBudget, ProjectionPlan,
    verified_projection_manifest_semantic_digest,
};
use search_revision_store::{
    DomainBinding,
    derived_artifact::{
        DerivedArtifactDescriptorV1 as Descriptor, DerivedArtifactErrorV1 as ArtifactError,
        DerivedArtifactKindV1 as Kind, DerivedArtifactPlaintextV1, DerivedOwnerProfileV1,
    },
};
use search_runtime_owner::OwnerError;
use search_unitizer::{
    UnitizationBudget, V3SourceBinding, ValidatedV3UnitizerProfile, VerifiedUnitSet,
    canonicalize_unit_manifest,
};

use super::{
    code_projection::{self, CodeProjectionError},
    raw_v2::{self, RawPreparationError},
};

/// Actual retained bytes, qualified owner inputs and original finite budgets.
/// IDs/domains remain data until Root joins their durable owner records.
pub struct RawCodeArtifactPreparation<'a> {
    pub source: &'a c::SourceRevisionRef,
    pub occurrence_sequence: u64,
    pub retained_body: &'a [u8],
    pub binding: &'a V3SourceBinding,
    pub request: &'a ValidatedMaterializationRequest,
    pub materializer: &'a ValidatedMaterializerProfile,
    pub unitizer: &'a ValidatedV3UnitizerProfile,
    pub materialization_budget: MaterializationBudget,
    pub unitization_budget: &'a UnitizationBudget<'a>,
    pub scope: &'a PlanningScope,
    pub schema: &'a CollectionSchema,
    pub lexical_profile: &'a AcceptedCodeIdentifiersProfile,
    pub language: &'a c::BoundedName,
    pub lexical_limits: CodeIdentifierLimits,
    pub sparse_limits: SparseLimits,
    pub projection_budget: ProjectionBudget,
    pub max_input_bytes: usize,
    pub max_total_vector_values: usize,
    pub max_artifact_bytes: usize,
    pub domains: &'a [DomainBinding; 6],
    pub operation: &'a c::OpaqueId,
    pub work: &'a dyn Fn(Work) -> Result<(), OwnerError>,
}

#[derive(Debug)]
pub enum RawArtifactPreparationError {
    Preparation(RawPreparationError),
    Projection(CodeProjectionError),
    Artifact(ArtifactError),
    Unitization(search_unitizer::UnitizationError),
    Materialization(search_materializer::api::MaterializationError),
    Context(OwnerError),
    ProfileMismatch,
    BudgetExceeded,
}

/// One owner-produced body, zeroized when the preparation is released.
pub struct PreparedRawArtifact {
    descriptor: Descriptor,
    body: DerivedArtifactPlaintextV1,
}
impl PreparedRawArtifact {
    pub const fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }
    pub fn body(&self) -> &[u8] {
        self.body.as_bytes()
    }
}

/// Private construction keeps producer outputs and exact C07 bodies together.
/// Neither mutable plans nor caller-assembled descriptor/body pairs escape.
pub struct PreparedRawCodeArtifacts {
    product: MaterializationProduct,
    units: VerifiedUnitSet,
    plan: ProjectionPlan,
    vectors: Vec<PlannedUnit>,
    artifacts: [PreparedRawArtifact; 4],
}
impl PreparedRawCodeArtifacts {
    pub const fn product(&self) -> &MaterializationProduct {
        &self.product
    }
    pub const fn units(&self) -> &VerifiedUnitSet {
        &self.units
    }
    pub const fn plan(&self) -> &ProjectionPlan {
        &self.plan
    }
    pub fn vectors(&self) -> &[PlannedUnit] {
        &self.vectors
    }
    pub const fn artifacts(&self) -> &[PreparedRawArtifact; 4] {
        &self.artifacts
    }

    /// C07's actual owner-payload check. Storage/root/crypto checks are separate.
    pub fn verify_owner_payload(
        &self,
        descriptor: &Descriptor,
        body: &[u8],
    ) -> Result<(), RawArtifactPreparationError> {
        let actual = self
            .artifacts
            .iter()
            .find(|artifact| artifact.descriptor.kind() == descriptor.kind())
            .ok_or(RawArtifactPreparationError::ProfileMismatch)?;
        if &actual.descriptor != descriptor || actual.body() != body {
            return Err(RawArtifactPreparationError::Artifact(
                ArtifactError::Mismatch,
            ));
        }
        Ok(())
    }
}

// Decode the full commitment already emitted in these owners' documented IDs.
// This creates neither a new hash scheme nor a UUID from a digest.
fn owned_profile_digest(
    value: &str,
    prefix: &str,
) -> Result<c::Blake3Digest32, RawArtifactPreparationError> {
    let encoded = value
        .strip_prefix(prefix)
        .filter(|value| value.len() == 64)
        .ok_or(RawArtifactPreparationError::ProfileMismatch)?;
    let digit = |byte| match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(RawArtifactPreparationError::ProfileMismatch),
    };
    let mut digest = [0; 32];
    for (index, pair) in encoded.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        digest[index] = digit(pair[0])? * 16 + digit(pair[1])?;
    }
    Ok(c::Blake3Digest32::from_bytes(digest))
}

fn artifact(
    input: &RawCodeArtifactPreparation<'_>,
    kind: Kind,
    profile: DerivedOwnerProfileV1,
    semantic: c::Blake3Digest32,
    body: &[u8],
) -> Result<PreparedRawArtifact, RawArtifactPreparationError> {
    if body.len() > input.max_artifact_bytes {
        return Err(RawArtifactPreparationError::BudgetExceeded);
    }
    let descriptor = Descriptor::from_encoded(
        kind,
        *input.source,
        input.binding.materialization_id,
        input.binding.representation_id,
        profile,
        semantic,
        input.domains.clone(),
        input.operation.clone(),
        body,
        input.max_artifact_bytes,
    )
    .map_err(RawArtifactPreparationError::Artifact)?;
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(body.len())
        .map_err(|_| RawArtifactPreparationError::BudgetExceeded)?;
    owned.extend_from_slice(body);
    Ok(PreparedRawArtifact {
        descriptor,
        body: DerivedArtifactPlaintextV1::new(owned),
    })
}

fn owner_profiles(
    input: &RawCodeArtifactPreparation<'_>,
    product: &MaterializationProduct,
    units: &VerifiedUnitSet,
) -> Result<[DerivedOwnerProfileV1; 3], RawArtifactPreparationError> {
    let projection_profile = code_projection::projection_profile_set(
        product,
        units,
        input.scope,
        input.schema,
        input.lexical_profile,
    )
    .map_err(RawArtifactPreparationError::Projection)?;
    let nonzero = |value| {
        c::NonZeroRevision::new(value).map_err(|_| RawArtifactPreparationError::ProfileMismatch)
    };
    let materializer_profile = DerivedOwnerProfileV1 {
        id: c::ProfileId::new(input.materializer.name())
            .map_err(|_| RawArtifactPreparationError::ProfileMismatch)?,
        revision: nonzero(input.materializer.revision())?,
        identity_digest: c::Blake3Digest32::from_bytes(*input.materializer.id().as_bytes()),
    };
    let unitizer_profile = DerivedOwnerProfileV1 {
        id: input.unitizer.id().clone(),
        revision: nonzero(input.unitizer.revision())?,
        identity_digest: owned_profile_digest(input.unitizer.id().as_str(), "unitizer-v3-")?,
    };
    let profile_set = projection_profile.profile_set_id();
    let projection_profile = DerivedOwnerProfileV1 {
        id: c::ProfileId::new(profile_set.as_str())
            .map_err(|_| RawArtifactPreparationError::ProfileMismatch)?,
        revision: nonzero(1)?,
        identity_digest: owned_profile_digest(profile_set.as_str(), "raw-code-c13-v1-")?,
    };
    Ok([materializer_profile, unitizer_profile, projection_profile])
}

/// Builds the four real owner bodies once, under the original caller allowance.
pub fn prepare_raw_code_artifacts(
    input: &RawCodeArtifactPreparation<'_>,
) -> Result<PreparedRawCodeArtifacts, RawArtifactPreparationError> {
    let checkpoint =
        || (input.work)(Work::Checkpoint).map_err(RawArtifactPreparationError::Context);
    checkpoint()?;
    if input.max_artifact_bytes == 0 || input.retained_body.len() > input.max_input_bytes {
        return Err(RawArtifactPreparationError::BudgetExceeded);
    }
    // Logical prepayment for producer encodings, four descriptor/body copies
    // and verification. Per-owner finite limits still bound actual output.
    let allocation = input
        .max_artifact_bytes
        .checked_mul(64)
        .and_then(|n| n.checked_add(16 * 32768))
        .ok_or(RawArtifactPreparationError::BudgetExceeded)?;
    for work in [Work::Allocate(allocation), Work::Process(allocation)] {
        (input.work)(work).map_err(RawArtifactPreparationError::Context)?;
    }
    let (product, units) = raw_v2::prepare_raw_v2(
        input.source,
        input.occurrence_sequence,
        input.retained_body,
        input.binding,
        input.request,
        input.materializer,
        input.unitizer,
        input.materialization_budget,
        input.unitization_budget,
    )
    .map_err(RawArtifactPreparationError::Preparation)?;
    checkpoint()?;
    let (plan, vectors) = code_projection::plan_raw_code_projection(
        &product,
        &units,
        input.scope,
        input.schema,
        input.lexical_profile,
        input.language,
        input.lexical_limits,
        input.sparse_limits,
        input.projection_budget,
        input.max_input_bytes,
        input.max_total_vector_values,
        input.unitization_budget.cancelled,
        input.unitization_budget.deadline,
    )
    .map_err(RawArtifactPreparationError::Projection)?;
    checkpoint()?;
    let materialization = canonicalize_materialization(&product)
        .map_err(RawArtifactPreparationError::Materialization)?;
    let unit_manifest = canonicalize_unit_manifest(units.manifest())
        .map_err(RawArtifactPreparationError::Unitization)?;
    let semantic = verified_projection_manifest_semantic_digest(
        &plan.manifest,
        &plan.points,
        input.scope,
        input.projection_budget,
    )
    .map_err(|error| {
        RawArtifactPreparationError::Projection(CodeProjectionError::Projection(error))
    })?;
    let [materializer_profile, unitizer_profile, projection_profile] =
        owner_profiles(input, &product, &units)?;
    let artifacts = [
        artifact(
            input,
            Kind::RepresentationBody,
            materializer_profile.clone(),
            product.representation_id(),
            product.canonical_text().as_bytes(),
        )?,
        artifact(
            input,
            Kind::MaterializationMetadata,
            materializer_profile,
            product.representation_id(),
            materialization.as_slice(),
        )?,
        artifact(
            input,
            Kind::UnitManifest,
            unitizer_profile,
            units.manifest_digest(),
            unit_manifest.as_slice(),
        )?,
        artifact(
            input,
            Kind::ProjectionManifest,
            projection_profile,
            semantic,
            &plan.manifest.canonical_bytes,
        )?,
    ];
    checkpoint()?;
    Ok(PreparedRawCodeArtifacts {
        product,
        units,
        plan,
        vectors,
        artifacts,
    })
}
