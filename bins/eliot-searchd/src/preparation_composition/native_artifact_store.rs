//! Root composition of owner-prepared artifacts into C07's native immutable store.
//!
//! This operation returns an actual C07 set write observation and a pure C02
//! prepared-set record candidate. It does not attach either to a visible epoch.

use search_control_redb::{
    JournalLimits, MigrationBootstrapWork as Work, PreparedArtifactSetMetadataV1,
    PreparedArtifactSetRecordV1,
};
use search_revision_store::derived_artifact::{
    DerivedArtifactErrorV1, DerivedArtifactObjectRefV1 as ObjectRef, DerivedArtifactSetDescriptorV1,
};

use super::artifact_payloads::{
    PreparedRawCodeArtifacts, RawArtifactPreparationError, RawCodeArtifactProducerInput,
    prepare_raw_code_artifacts_from_native,
};
use crate::owner_composition::NativeArtifactStoreAdapter;

/// Completed observation and candidate metadata from one actual Root operation.
#[allow(dead_code)] // The ordinary factory caller remains outside this source slice.
pub struct NativeStoredArtifactSet {
    write: crate::owner_composition::NativeArtifactSetWrite,
    record: PreparedArtifactSetRecordV1,
    prepared: PreparedRawCodeArtifacts,
}

#[allow(dead_code)] // Consumed by the parent-owned factory caller after cutover.
impl NativeStoredArtifactSet {
    /// Actual C07 shared-kernel set write and exact final-object readback.
    pub(crate) const fn c07_write(&self) -> &crate::owner_composition::NativeArtifactSetWrite {
        &self.write
    }

    /// Pure C02 record candidate rooted in the actual C07 set bytes.
    pub(crate) const fn c02_candidate(&self) -> &PreparedArtifactSetRecordV1 {
        &self.record
    }

    /// Exact owner outputs used to create this stored generation.
    pub(crate) const fn prepared(&self) -> &PreparedRawCodeArtifacts {
        &self.prepared
    }
}

#[allow(dead_code)] // Returned to the parent-owned factory caller after cutover.
pub enum NativeArtifactCompositionError {
    Preparation(RawArtifactPreparationError),
    Native(crate::owner_composition::NativeArtifactStoreError),
    Set(DerivedArtifactErrorV1),
    Record(search_control_redb::ControlError),
}

#[allow(dead_code)] // Parent inspects possible effects after its factory cutover.
impl NativeArtifactCompositionError {
    pub(crate) const fn outcome_unknown(&self) -> bool {
        match self {
            Self::Native(error) => error.outcome_unknown(),
            Self::Preparation(_) | Self::Set(_) | Self::Record(_) => false,
        }
    }
}

/// Prepare the four actual owner bodies, publish them in their closed C07 order,
/// then publish C07's own four-member set object under the same Native context.
pub fn prepare_and_store(
    adapter: &mut NativeArtifactStoreAdapter<'_, '_, '_, '_, '_>,
    producer: &RawCodeArtifactProducerInput<'_>,
    before_publish: impl FnOnce(
        &PreparedRawCodeArtifacts,
    ) -> Result<(), NativeArtifactCompositionError>,
) -> Result<NativeStoredArtifactSet, NativeArtifactCompositionError> {
    let source = *adapter.source_revision();
    let operation = adapter.operation_id().clone();
    let prepared = {
        let sequence = adapter.occurrence_sequence();
        let body = adapter.retained_source_body();
        let domains = adapter.domains();
        let work = |unit| adapter.work(unit);
        prepare_raw_code_artifacts_from_native(
            producer, &source, sequence, body, domains, &operation, &work,
        )
        .map_err(NativeArtifactCompositionError::Preparation)?
    };
    if prepared.scope() != adapter.scope() || prepared.schema() != adapter.schema() {
        return Err(NativeArtifactCompositionError::Native(
            crate::owner_composition::NativeArtifactStoreError::OwnerRefused,
        ));
    }
    adapter
        .verify_prepared(&prepared)
        .map_err(NativeArtifactCompositionError::Native)?;
    before_publish(&prepared)?;

    let artifacts = prepared.artifacts();
    let first = adapter
        .publish_member(&prepared, &artifacts[0])
        .map_err(NativeArtifactCompositionError::Native)?;
    let second = adapter
        .publish_member(&prepared, &artifacts[1])
        .map_err(NativeArtifactCompositionError::Native)?;
    let third = adapter
        .publish_member(&prepared, &artifacts[2])
        .map_err(NativeArtifactCompositionError::Native)?;
    let fourth = adapter
        .publish_member(&prepared, &artifacts[3])
        .map_err(NativeArtifactCompositionError::Native)?;

    let members: [ObjectRef; 4] = [
        first.object_ref().clone(),
        second.object_ref().clone(),
        third.object_ref().clone(),
        fourth.object_ref().clone(),
    ];
    let set = DerivedArtifactSetDescriptorV1::new(members)
        .map_err(NativeArtifactCompositionError::Set)?;
    // The descriptor and unqualified candidate both come from C07's own codec.
    let set_candidate = set
        .candidate(operation.clone())
        .map_err(NativeArtifactCompositionError::Set)?;
    let write = adapter
        .publish_set(&prepared, &set, &set_candidate)
        .map_err(NativeArtifactCompositionError::Native)?;

    let reference = write.object_ref();
    let descriptor = reference.descriptor();
    if *descriptor.source() != source
        || descriptor.operation_id() != &operation
        || descriptor.materialization_id() != adapter.materialization_id()
        || descriptor.representation_id() != adapter.representation_id()
    {
        return Err(NativeArtifactCompositionError::Native(
            crate::owner_composition::NativeArtifactStoreError::OwnerRefused,
        ));
    }
    let metadata = PreparedArtifactSetMetadataV1 {
        operation_id: descriptor.operation_id().clone(),
        source_revision: *descriptor.source(),
        materialization_id: descriptor.materialization_id(),
        representation_id: descriptor.representation_id(),
        residency_canonical: adapter
            .residency_canonical()
            .map_err(NativeArtifactCompositionError::Native)?,
        object_reference_canonical: reference
            .encode()
            .map_err(|_| NativeArtifactCompositionError::Set(DerivedArtifactErrorV1::Mismatch))?,
        stored_object_digest: reference.stored_object_digest(),
        stored_object_length: reference.stored_bytes(),
    };
    let record = {
        let deadline = adapter.deadline();
        let work = |unit: Work| adapter.work(unit);
        PreparedArtifactSetRecordV1::prepare_with_original_context(
            metadata,
            JournalLimits::BASELINE,
            deadline,
            &work,
        )
        .map_err(|failure| NativeArtifactCompositionError::Record(failure.control))?
    };
    adapter
        .final_checkpoint()
        .map_err(NativeArtifactCompositionError::Native)?;

    Ok(NativeStoredArtifactSet {
        write,
        record,
        prepared,
    })
}
