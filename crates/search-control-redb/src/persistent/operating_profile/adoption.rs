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
        let result = (|| {
            if self.identity.schema_version != crate::REGISTRY_JOURNAL_SCHEMA_VERSION {
                return Err(ControlError::SchemaUnsupported);
            }
            let snapshot = self.read_snapshot_checked(&budget)?;
            let profile = snapshot
                .get(&ControlKey::new(super::KEY.to_vec(), self.limits)?)
                .ok_or(ControlError::SchemaMismatch)?;
            if Record::decode(profile, self.limits)?.initial != JournalProfileInitialization::Direct
            {
                return Err(ControlError::MigrationUnverified);
            }
            if let Some(value) = snapshot.get(&ControlKey::new(KEY.to_vec(), self.limits)?) {
                let original = IndexedProfileAdoption::decode(value, self.limits)?;
                if original.operation != operation
                    || original.digest != digest
                    || original.initial != initial
                {
                    return Err(ControlError::OperationConflict);
                }
                return Ok(original);
            }
            let request = IndexedProfileAdoption {
                operation,
                digest,
                epoch: self.identity.owner_epoch,
                generation: snapshot.generation,
                direct: profile.clone(),
                initial,
            };
            request.command(self.limits)?;
            budget.check(Point::ReadComplete)?;
            Ok(request)
        })();
        result.map_err(|error| budget.failure(error, Some(operation)))
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

    pub(super) fn adopt_indexed_checked(
        &mut self,
        request: &IndexedProfileAdoption,
        boundary: Boundary,
        check: &dyn Check,
    ) -> Result<ControlCommitReceipt, ControlError> {
        if self.identity.schema_version != crate::REGISTRY_JOURNAL_SCHEMA_VERSION {
            return Err(ControlError::SchemaUnsupported);
        }
        if request.epoch != self.identity.owner_epoch {
            return Err(ControlError::GenerationMismatch);
        }
        let command = request.command(self.limits)?;
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
        let result = (|| {
            budget.check(Point::Start)?;
            if self.identity.schema_version != crate::REGISTRY_JOURNAL_SCHEMA_VERSION {
                return Err(ControlError::SchemaUnsupported);
            }
            if request.epoch != self.identity.owner_epoch {
                return Err(ControlError::GenerationMismatch);
            }
            let command = request.command(self.limits)?;
            self.recover_transaction_with_conditions_checked(
                command.mutation(),
                command.conditions(),
                &budget,
            )
        })();
        result.map_err(|error| {
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
    let request = IndexedProfileAdoption::decode(value, journal.limits)?;
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
    let command = request.command(journal.limits)?;
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
