//! Retained projection coordinates, without representation or access authority.

use crate::{error::RegistryError, membership::PermanentMembershipProvenanceV2};
use search_contracts::{
    BoundedBytes, MAX_CANONICAL_BYTES, MembershipRevision, ProjectionMembership, ReceiptRef,
    parse_canonical_cbor, to_canonical_cbor,
};

/// Maximum canonical meaning retained for one shared partition class.
pub const MAX_PROJECTION_PARTITION_SEMANTICS_BYTES: usize = 8 * 1024;

/// Source-independent full meanings for access and scoring partition IDs.
///
/// Root excludes source/membership/operation identities before construction.
/// C03 stores these bytes with the first-bind record and only reuses a class
/// ID for byte-identical canonical meaning.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionPartitionSemantics {
    access: BoundedBytes<MAX_PROJECTION_PARTITION_SEMANTICS_BYTES>,
    scoring: BoundedBytes<MAX_PROJECTION_PARTITION_SEMANTICS_BYTES>,
}

impl ProjectionPartitionSemantics {
    /// Validate and retain two bounded canonical-CBOR class meanings.
    pub fn new(access: Vec<u8>, scoring: Vec<u8>) -> Result<Self, RegistryError> {
        Ok(Self {
            access: canonical_semantics(access)?,
            scoring: canonical_semantics(scoring)?,
        })
    }

    /// Complete canonical access-class meaning.
    pub fn access(&self) -> &[u8] {
        self.access.as_slice()
    }

    /// Complete canonical scoring-class meaning.
    pub fn scoring(&self) -> &[u8] {
        self.scoring.as_slice()
    }
}

fn canonical_semantics(
    value: Vec<u8>,
) -> Result<BoundedBytes<MAX_PROJECTION_PARTITION_SEMANTICS_BYTES>, RegistryError> {
    if value.is_empty() || value.len() > MAX_PROJECTION_PARTITION_SEMANTICS_BYTES {
        return Err(RegistryError::SnapshotInvalid);
    }
    let decoded = parse_canonical_cbor(&value).map_err(|_| RegistryError::SnapshotInvalid)?;
    let canonical = to_canonical_cbor(&decoded).map_err(|_| RegistryError::SnapshotInvalid)?;
    if canonical.as_slice() != value.as_slice() || value.len() > MAX_CANONICAL_BYTES {
        return Err(RegistryError::SnapshotInvalid);
    }
    BoundedBytes::new(value).map_err(|_| RegistryError::SnapshotInvalid)
}

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
    /// Full source-independent class meanings for new qualified C03 bindings.
    /// `None` is retained only for historical/legacy first binds.
    pub partition_semantics: Option<ProjectionPartitionSemantics>,
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
        if let Some(semantics) = &self.partition_semantics {
            canonical_semantics(semantics.access().to_vec())?;
            canonical_semantics(semantics.scoring().to_vec())?;
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
