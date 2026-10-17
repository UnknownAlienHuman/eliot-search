//! Retained projection coordinates, without representation or access authority.

use crate::{error::RegistryError, membership::PermanentMembershipProvenanceV2};
use search_contracts::{MembershipRevision, ProjectionMembership, ReceiptRef};

/// Exact upstream coordinates and C03 predecessor observations for a first bind.
///
/// Root must supply the actual projection UUID and qualified representation,
/// partition and schema coordinates. These ordinary values do not prove their
/// origin, representation completeness, access permission or scoring meaning.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionMembershipBindRequest {
    /// Complete C00 binding retained without identity conversion.
    pub binding: ProjectionMembership,
    /// Exact permanent member revision observed by the caller.
    pub expected_membership_revision: MembershipRevision,
    /// Exact source/root/namespace/policy references and generations.
    pub expected_provenance: PermanentMembershipProvenanceV2,
}

impl ProjectionMembershipBindRequest {
    /// Validate bounded metadata shape only; no owner qualification is issued.
    pub fn validate_shape(&self) -> Result<(), RegistryError> {
        let b = &self.binding;
        if self.expected_membership_revision.get() == 0
            || [
                b.projection_membership_id.as_bytes(),
                b.source_membership_id.as_bytes(),
                b.representation_id.as_bytes(),
                b.access_partition_id.as_bytes(),
                b.scoring_partition_id.as_bytes(),
            ]
            .iter()
            .any(|id| id.iter().all(|byte| *byte == 0))
        {
            return Err(RegistryError::SnapshotInvalid);
        }
        Ok(())
    }
}

/// Immutable first-bind metadata; exact restoration never grants live authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionMembershipRecord {
    request: ProjectionMembershipBindRequest,
    registry_revision: u64,
    receipt: ReceiptRef,
}

impl ProjectionMembershipRecord {
    /// Restore plain observed metadata. C03 must still verify its relationships.
    pub fn from_observed_parts(
        request: ProjectionMembershipBindRequest,
        registry_revision: u64,
        receipt: ReceiptRef,
    ) -> Result<Self, RegistryError> {
        request.validate_shape()?;
        if registry_revision == 0 {
            return Err(RegistryError::SnapshotInvalid);
        }
        Ok(Self {
            request,
            registry_revision,
            receipt,
        })
    }

    /// Exact original coordinates and predecessor fences.
    pub const fn request(&self) -> &ProjectionMembershipBindRequest {
        &self.request
    }
    /// Exact C00 projection membership; not an access or representation proof.
    pub const fn binding(&self) -> &ProjectionMembership {
        &self.request.binding
    }
    /// Registry revision of the original first bind.
    pub const fn registry_revision(&self) -> u64 {
        self.registry_revision
    }
    /// Original content-free durability receipt.
    pub const fn receipt(&self) -> &ReceiptRef {
        &self.receipt
    }
}
