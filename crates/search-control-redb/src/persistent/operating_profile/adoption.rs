//! Exact one-way technical adoption; the original generation-zero profile stays immutable.
use super::{
    Blake3Digest32, Boundary, Budget, CancellationProbe, Check, CommitRecoveryDecision,
    ConditionalControlMutation, ControlCallError, ControlCommitReceipt, ControlError, ControlKey,
    ControlMutation, ControlRecordClass, ControlRecordCondition, ControlValue, ControlWrite,
    JournalIdentity, JournalLimits, JournalOperatingProfile, JournalProfileInitialization,
    JournalReadSnapshot, MutationId, OperationContext, OwnerEpoch, PersistentControlJournal, Point,
    PublicationVisibilityState, ReadTransaction, Record, bind_conditions, operation_from_checked,
    request_fingerprint,
};

pub(in crate::persistent) const KEY: &[u8] = b"eliot.control.indexed-adoption.v1";
const MAGIC: &[u8; 8] = b"ELJADP01";
// The adoption and visibility codecs contain fixed-width metadata only. This
// bound covers their buffers, command vectors/keys, copies and validation work.
const COMMAND_WORK_BYTES: usize = 16 * 1024;

/// Prepared original command captured from one coherent journal snapshot.
///
/// This binds technical state only; Root must independently verify the Qdrant
/// artifact, schema, route, source/access guards and qualification before preparation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedProfileAdoption {
    operation: MutationId,
    digest: Blake3Digest32,
    epoch: OwnerEpoch,
    generation: u64,
    direct: ControlValue,
    initial: PublicationVisibilityState,
}

impl IndexedProfileAdoption {
    /// Exact original operation retained on disk; never a replacement identity.
    #[must_use]
    pub const fn operation_id(&self) -> MutationId {
        self.operation
    }

    /// Original caller command digest retained with the complete adoption.
    #[must_use]
    pub const fn command_digest(&self) -> Blake3Digest32 {
        self.digest
    }

    /// Exact seed retained by this original command, not current indexed readiness.
    #[must_use]
    pub const fn initial_visibility(&self) -> PublicationVisibilityState {
        self.initial
    }

    /// Exact compare-and-swap predecessor captured before dispatch.
    #[must_use]
    pub const fn expected_generation(&self) -> u64 {
        self.generation
    }

    fn value(&self, limits: JournalLimits) -> Result<ControlValue, ControlError> {
        let direct = Record::decode(&self.direct, limits)?;
        if direct.initial != JournalProfileInitialization::Direct
            || direct.epoch > self.epoch
            || self.generation == 0
            || self.initial.guards.owner_epoch != self.epoch
            || self.initial.visible_epoch.get() != 0
            || self.initial.last_receipt.is_some()
        {
            return Err(ControlError::InvalidValue);
        }
        let mut out = MAGIC.to_vec();
        out.extend_from_slice(&self.operation.0);
        out.extend_from_slice(self.digest.as_bytes());
        out.extend_from_slice(&self.epoch.get().to_be_bytes());
        out.extend_from_slice(&self.generation.to_be_bytes());
        out.extend_from_slice(self.direct.as_bytes());
        out.extend_from_slice(
            super::super::publication::visibility::codec::state(&self.initial, limits)?.as_bytes(),
        );
        ControlValue::new(ControlRecordClass::State, out, limits)
    }

    pub(super) fn decode(
        value: &ControlValue,
        limits: JournalLimits,
    ) -> Result<Self, ControlError> {
        let b = value.as_bytes();
        if value.class() != ControlRecordClass::State || b.len() < 137 || &b[..8] != MAGIC {
            return Err(ControlError::StoreCorrupt);
        }
        let original = ControlValue::new(ControlRecordClass::State, b[88..137].to_vec(), limits)
            .map_err(|_| ControlError::StoreCorrupt)?;
        let state = ControlValue::new(ControlRecordClass::Revision, b[137..].to_vec(), limits)
            .map_err(|_| ControlError::StoreCorrupt)?;
        let request = Self {
            operation: MutationId(
                b[8..40]
                    .try_into()
                    .map_err(|_| ControlError::StoreCorrupt)?,
            ),
            digest: Blake3Digest32::from_stored_bytes(
                b[40..72]
                    .try_into()
                    .map_err(|_| ControlError::StoreCorrupt)?,
            ),
            epoch: OwnerEpoch::new(u64::from_be_bytes(
                b[72..80]
                    .try_into()
                    .map_err(|_| ControlError::StoreCorrupt)?,
            ))
            .map_err(|_| ControlError::StoreCorrupt)?,
            generation: u64::from_be_bytes(
                b[80..88]
                    .try_into()
                    .map_err(|_| ControlError::StoreCorrupt)?,
            ),
            direct: original,
            initial: super::super::publication::visibility::codec::read_state(&state)?,
        };
        if request
            .value(limits)
            .map_err(|_| ControlError::StoreCorrupt)?
            != *value
        {
            return Err(ControlError::StoreCorrupt);
        }
        Ok(request)
    }

    fn command(&self, limits: JournalLimits) -> Result<ConditionalControlMutation, ControlError> {
        let adoption_key = ControlKey::new(KEY.to_vec(), limits)?;
        let profile_key = ControlKey::new(super::KEY.to_vec(), limits)?;
        let state_key = ControlKey::new(
            super::super::publication::visibility::STATE.to_vec(),
            limits,
        )?;
        Ok(ConditionalControlMutation::new(
            ControlMutation::new(
                self.operation,
                self.digest,
                self.generation,
                vec![
                    ControlWrite {
                        key: adoption_key.clone(),
                        value: self.value(limits)?,
                    },
                    ControlWrite {
                        key: state_key.clone(),
                        value: super::super::publication::visibility::codec::state(
                            &self.initial,
                            limits,
                        )?,
                    },
                ],
                vec![],
            ),
            vec![
                ControlRecordCondition::exact(profile_key, self.direct.clone()),
                ControlRecordCondition::absent(adoption_key),
                ControlRecordCondition::absent(state_key),
                ControlRecordCondition::absent(ControlKey::new(
                    super::super::publication::KEY.to_vec(),
                    limits,
                )?),
            ],
        ))
    }

    fn command_checked(
        &self,
        limits: JournalLimits,
        check: &dyn Check,
    ) -> Result<ConditionalControlMutation, ControlError> {
        check.reserve(crate::MigrationBootstrapWork::Allocate(COMMAND_WORK_BYTES))?;
        check.reserve(crate::MigrationBootstrapWork::Process(COMMAND_WORK_BYTES))?;
        self.command(limits)
    }

    fn decode_checked(
        value: &ControlValue,
        limits: JournalLimits,
        check: &dyn Check,
    ) -> Result<Self, ControlError> {
        let work = value
            .len()
            .checked_mul(16)
            .ok_or(ControlError::BudgetExceeded)?;
        check.reserve(crate::MigrationBootstrapWork::Allocate(work))?;
        check.reserve(crate::MigrationBootstrapWork::Process(work))?;
        Self::decode(value, limits)
    }
}

impl PersistentControlJournal {
    /// Captures the exact DIRECT predecessor and generation without effects.
    /// An existing identical adoption returns its original command before any
    /// caller may allocate another collection identity or change the seed.
    /// # Errors
    /// Non-DIRECT roots, substituted inputs, uninitialized state and interruption refuse.
    pub fn prepare_indexed_profile_adoption<C: CancellationProbe>(
        &self,
        initial: PublicationVisibilityState,
        operation: MutationId,
        digest: Blake3Digest32,
        context: &OperationContext<C>,
    ) -> Result<IndexedProfileAdoption, ControlCallError> {
        let budget = Budget::new(context);
        self.prepare_indexed_profile_adoption_checked(initial, operation, digest, &budget)
            .map_err(|error| budget.failure(error, Some(operation)))
    }

    pub(in crate::persistent) fn prepare_indexed_profile_adoption_checked(
        &self,
        initial: PublicationVisibilityState,
        operation: MutationId,
        digest: Blake3Digest32,
        check: &dyn Check,
    ) -> Result<IndexedProfileAdoption, ControlError> {
        check.check(Point::Start)?;
        if self.identity.schema_version != crate::REGISTRY_JOURNAL_SCHEMA_VERSION {
            return Err(ControlError::SchemaUnsupported);
        }
        let snapshot = self.read_snapshot_checked(check)?;
        check.reserve(crate::MigrationBootstrapWork::Allocate(COMMAND_WORK_BYTES))?;
        check.reserve(crate::MigrationBootstrapWork::Process(COMMAND_WORK_BYTES))?;
        let profile = snapshot
            .get(&ControlKey::new(super::KEY.to_vec(), self.limits)?)
            .ok_or(ControlError::SchemaMismatch)?;
        if Record::decode(profile, self.limits)?.initial != JournalProfileInitialization::Direct {
            return Err(ControlError::MigrationUnverified);
        }
        if let Some(value) = snapshot.get(&ControlKey::new(KEY.to_vec(), self.limits)?) {
            let original = IndexedProfileAdoption::decode_checked(value, self.limits, check)?;
            if original.operation != operation
                || original.digest != digest
                || original.initial != initial
            {
                return Err(ControlError::OperationConflict);
            }
            check.check(Point::ReadComplete)?;
            return Ok(original);
        }
        check.reserve(crate::MigrationBootstrapWork::Allocate(profile.len()))?;
        check.reserve(crate::MigrationBootstrapWork::Process(profile.len()))?;
        let request = IndexedProfileAdoption {
            operation,
            digest,
            epoch: self.identity.owner_epoch,
            generation: snapshot.generation,
            direct: profile.clone(),
            initial,
        };
        request.command_checked(self.limits, check)?;
        check.check(Point::ReadComplete)?;
        Ok(request)
    }

    pub(in crate::persistent) fn read_indexed_profile_adoption_checked(
        &self,
        check: &dyn Check,
    ) -> Result<Option<IndexedProfileAdoption>, ControlError> {
        check.check(Point::Start)?;
        if self.identity.schema_version != crate::REGISTRY_JOURNAL_SCHEMA_VERSION {
            return Err(ControlError::SchemaUnsupported);
        }
        let snapshot = self.read_snapshot_checked(check)?;
        check.reserve(crate::MigrationBootstrapWork::Allocate(COMMAND_WORK_BYTES))?;
        check.reserve(crate::MigrationBootstrapWork::Process(COMMAND_WORK_BYTES))?;
        let profile = snapshot
            .get(&ControlKey::new(super::KEY.to_vec(), self.limits)?)
            .ok_or(ControlError::SchemaMismatch)?;
        if Record::decode(profile, self.limits)?.initial != JournalProfileInitialization::Direct {
            return Err(ControlError::MigrationUnverified);
        }
        let original = snapshot
            .get(&ControlKey::new(KEY.to_vec(), self.limits)?)
            .map(|value| IndexedProfileAdoption::decode_checked(value, self.limits, check))
            .transpose()?;
        check.check(Point::ReadComplete)?;
        Ok(original)
    }

    /// Atomically adds the original adoption and epoch-zero visibility, leaving
    /// immutable profile/source metadata intact. Publish the new snapshot separately.
    /// # Errors
    /// Stale generation/owner or conflicting seed refuses; lost acknowledgement remains unknown.
    pub fn adopt_indexed_profile<C: CancellationProbe>(
        &mut self,
        request: &IndexedProfileAdoption,
        context: &OperationContext<C>,
    ) -> Result<ControlCommitReceipt, ControlCallError> {
        let budget = Budget::new(context);
        self.adopt_indexed_checked(request, Boundary::Normal, &budget)
            .map_err(|error| budget.failure(error, Some(request.operation)))
    }

    pub(in crate::persistent) fn adopt_indexed_checked(
        &mut self,
        request: &IndexedProfileAdoption,
        boundary: Boundary,
        check: &dyn Check,
    ) -> Result<ControlCommitReceipt, ControlError> {
        check.check(Point::Start)?;
        if self.identity.schema_version != crate::REGISTRY_JOURNAL_SCHEMA_VERSION {
            return Err(ControlError::SchemaUnsupported);
        }
        if request.epoch != self.identity.owner_epoch {
            return Err(ControlError::GenerationMismatch);
        }
        let command = request.command_checked(self.limits, check)?;
        super::super::transaction::execute_profile(
            self,
            command.mutation(),
            command.conditions(),
            boundary,
            check,
        )
    }

    /// Resolves the exact original uncertain command without replaying an effect.
    /// # Errors
    /// Different input cannot clear pending work; interrupted recovery remains unknown.
    pub fn recover_indexed_profile_adoption<C: CancellationProbe>(
        &mut self,
        request: &IndexedProfileAdoption,
        context: &OperationContext<C>,
    ) -> Result<CommitRecoveryDecision, ControlCallError> {
        let budget = Budget::new(context);
        self.recover_indexed_profile_adoption_checked(request, &budget)
            .map_err(|error| {
                budget
                    .failure(
                        if budget.interrupted() {
                            ControlError::CommitOutcomeUnknown
                        } else {
                            error
                        },
                        Some(request.operation),
                    )
                    .for_recovery()
            })
    }

    pub(in crate::persistent) fn recover_indexed_profile_adoption_checked(
        &mut self,
        request: &IndexedProfileAdoption,
        check: &dyn Check,
    ) -> Result<CommitRecoveryDecision, ControlError> {
        check.check(Point::Start)?;
        if self.identity.schema_version != crate::REGISTRY_JOURNAL_SCHEMA_VERSION {
            return Err(ControlError::SchemaUnsupported);
        }
        if request.epoch != self.identity.owner_epoch {
            return Err(ControlError::GenerationMismatch);
        }
        let command = request.command_checked(self.limits, check)?;
        self.recover_transaction_with_conditions_checked(
            command.mutation(),
            command.conditions(),
            check,
        )
    }
}

pub(super) fn effective(
    initial: JournalOperatingProfile,
    value: Option<&ControlValue>,
    limits: JournalLimits,
) -> Result<JournalOperatingProfile, ControlError> {
    match (initial, value) {
        (JournalOperatingProfile::Direct, Some(value)) => {
            IndexedProfileAdoption::decode(value, limits)?;
            Ok(JournalOperatingProfile::Indexed)
        }
        (_, None) => Ok(initial),
        _ => Err(ControlError::StoreCorrupt),
    }
}

pub(super) fn validate(
    journal: &PersistentControlJournal,
    read: &ReadTransaction,
    snapshot: &JournalReadSnapshot,
    check: &dyn Check,
) -> Result<(), ControlError> {
    let Some(value) = snapshot.get(&ControlKey::new(KEY.to_vec(), journal.limits)?) else {
        return Ok(());
    };
    let request = IndexedProfileAdoption::decode_checked(value, journal.limits, check)?;
    if request.epoch > snapshot.identity.owner_epoch
        || snapshot.get(&ControlKey::new(super::KEY.to_vec(), journal.limits)?)
            != Some(&request.direct)
    {
        return Err(ControlError::StoreCorrupt);
    }
    let header = journal.header_from_checked(read, check)?;
    let original = JournalIdentity {
        owner_epoch: request.epoch,
        ..snapshot.identity
    };
    let operation =
        operation_from_checked(read, request.operation, &header, journal.limits, check)?
            .ok_or(ControlError::StoreCorrupt)?;
    let command = request.command_checked(journal.limits, check)?;
    let fingerprint = bind_conditions(
        request_fingerprint(original, command.mutation())?,
        command.conditions(),
        || check.check(Point::ReadRecord),
    )?;
    if operation.request_sha256 != fingerprint {
        return Err(ControlError::StoreCorrupt);
    }
    let mut keys = command
        .mutation()
        .writes()
        .iter()
        .map(|w| w.key.clone())
        .collect::<Vec<_>>();
    keys.sort();
    super::super::transaction::verify_replay(
        journal,
        read,
        &header,
        &operation.receipt,
        command.mutation(),
        &keys,
        check,
    )
}

#[cfg(test)]
mod original_context_tests {
    use super::*;
    use crate::persistent::bootstrap::OriginalCheck;
    use crate::{ControlSnapshotPublisher, MigrationBootstrapWork as Work};
    use search_contracts::{
        CollectionGenerationId, DataRootId, Epoch, InstallationIncarnationId, OpaqueRef,
        PublicationGuards, RequestId,
    };
    use search_ports::{ControlSnapshotPort, PackageOpaque};
    use std::{
        cell::Cell,
        fs::{File, OpenOptions},
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        time::{Duration, Instant},
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Scratch(PathBuf);
    impl Scratch {
        fn create() -> (Self, File) {
            let path = std::env::temp_dir().join(format!(
                "eliot-indexed-adoption-original-{}-{}.redb",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap();
            (Self(path), file)
        }
        fn reopen(&self) -> File {
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(&self.0)
                .unwrap()
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    #[derive(Debug)]
    struct Cancel;
    impl PackageOpaque for Cancel {
        fn owner_package(&self) -> &'static str {
            "search-control-redb"
        }
    }
    impl CancellationProbe for Cancel {
        fn is_cancelled(&self) -> bool {
            false
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One disk case keeps original preparation, unknown, recovery and reopen causal.
    fn original_adoption_port_retains_inputs_and_recovers_same_handle() {
        let (scratch, file) = Scratch::create();
        let limits = JournalLimits::BASELINE;
        let epoch = OwnerEpoch::new(1).unwrap();
        let digest = Blake3Digest32::from_bytes([4; 32]);
        let identity = JournalIdentity {
            installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
            data_root_id: DataRootId::from_bytes([2; 16]),
            owner_epoch: epoch,
            path_identity_digest: digest,
            schema_family_digest: digest,
            schema_version: crate::REGISTRY_JOURNAL_SCHEMA_VERSION,
        };
        let initial = PublicationVisibilityState {
            collection_generation_id: CollectionGenerationId::from_bytes([3; 16]),
            schema_identity_digest: digest,
            visible_epoch: Epoch::new(0).unwrap(),
            guards: PublicationGuards {
                owner_epoch: epoch,
                source_catalog_generation: 0,
                membership_generation: 0,
                access_generation: 0,
                shadow_generation: 0,
                purge_generation: 0,
                profile_digest: digest,
            },
            last_receipt: None,
        };
        let context = OperationContext::new(
            RequestId::from_bytes([5; 16]),
            60_000,
            Cancel,
            OpaqueRef::new("budget:original-indexed-adoption").unwrap(),
        )
        .unwrap();
        let mut journal = PersistentControlJournal::create(file, identity, limits).unwrap();
        journal
            .initialize_operating_profile(
                JournalProfileInitialization::Direct,
                MutationId([6; 32]),
                digest,
                &context,
            )
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(60);
        let allocations = Cell::new(0_usize);
        let work = |unit| {
            if let Work::Allocate(bytes) = unit {
                allocations.set(allocations.get() + bytes);
            }
            Ok::<(), &'static str>(())
        };
        let mut publisher = ControlSnapshotPublisher::new();
        let request = {
            let port = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
            assert_eq!(
                port.read_indexed_profile_adoption_with_original_context(deadline, &work)
                    .unwrap(),
                None
            );
            assert!(
                port.read_indexed_profile_adoption_with_original_context(
                    Instant::now(),
                    &|_| -> Result<(), ()> { panic!("expired context must refuse before work") }
                )
                .is_err()
            );
            assert!(
                port.prepare_indexed_profile_adoption_with_original_context(
                    initial,
                    MutationId([7; 32]),
                    digest,
                    deadline,
                    &|_| Err("original allowance refused")
                )
                .is_err()
            );
            port.prepare_indexed_profile_adoption_with_original_context(
                initial,
                MutationId([7; 32]),
                digest,
                deadline,
                &work,
            )
            .unwrap()
        };
        assert_eq!(request.expected_generation(), 1);
        assert_eq!(request.operation_id(), MutationId([7; 32]));
        assert_eq!(request.command_digest(), digest);
        assert!(allocations.get() > 0);
        assert_eq!(
            journal
                .adopt_indexed_checked(
                    &request,
                    Boundary::LostAcknowledgement,
                    &OriginalCheck::registry(deadline, &work)
                )
                .unwrap_err(),
            ControlError::CommitOutcomeUnknown
        );
        {
            let mut port = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
            assert!(
                port.read_indexed_profile_adoption_with_original_context(deadline, &work)
                    .is_err()
            );
            let mut changed = request.clone();
            changed.initial.collection_generation_id = CollectionGenerationId::from_bytes([8; 16]);
            assert_eq!(
                port.recover_indexed_profile_adoption_with_original_context(
                    &changed, deadline, &work
                )
                .unwrap(),
                CommitRecoveryDecision::ConflictingInput
            );
            assert!(
                port.recover_indexed_profile_adoption_with_original_context(
                    &request,
                    deadline,
                    &|_| Err("cancelled recovery")
                )
                .is_err()
            );
            assert!(matches!(
                port.recover_indexed_profile_adoption_with_original_context(
                    &request, deadline, &work
                )
                .unwrap(),
                CommitRecoveryDecision::Committed(_)
            ));
            assert!(
                port.adopt_indexed_profile_with_original_context(&request, deadline, &work)
                    .unwrap()
                    .replayed
            );
            assert!(port.current_snapshot().is_err());
        }
        drop(journal);
        let mut reopened =
            PersistentControlJournal::open(scratch.reopen(), identity, limits).unwrap();
        let mut publisher = ControlSnapshotPublisher::new();
        let port = reopened
            .bind_control_port::<Cancel>(&mut publisher)
            .unwrap();
        let original = port
            .read_indexed_profile_adoption_with_original_context(deadline, &work)
            .unwrap()
            .unwrap();
        assert_eq!(original, request);
        assert_eq!(
            original.initial_visibility().collection_generation_id,
            initial.collection_generation_id
        );
        assert_eq!(
            port.prepare_indexed_profile_adoption_with_original_context(
                initial,
                request.operation_id(),
                digest,
                deadline,
                &work
            )
            .unwrap(),
            request
        );
        assert!(
            port.prepare_indexed_profile_adoption_with_original_context(
                initial,
                MutationId([9; 32]),
                digest,
                deadline,
                &work
            )
            .is_err()
        );
    }
}
