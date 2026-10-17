//! One closed fresh DIRECT root/journal owner. No migration adoption or legacy store.
use super::super::native_objects::DirectPin;
use super::super::native_objects::native_identity;
use super::super::storage_acl::StorageAclSnapshot;
use super::{
    AcquiredNativeRoot, NativeRootCapability, NativeRootRequest, NativeWork,
    acquire_existing_direct, finish_in_place,
};
use search_contracts::{Blake3Digest32, DataRootId, InstallationIncarnationId, OwnerEpoch};
use search_control_redb::{
    ControlError, ControlSnapshotPublisher, JournalLimits, JournalOwnerBinding,
    MigrationBootstrapFailure, MigrationBootstrapWork as W, MutationId, PersistentControlJournal,
};
use search_runtime_owner::{DrainReason, OwnerError};
use std::{cell::RefCell, fs::File, io::Write, path::Path};

pub(in super::super) struct DirectJournal {
    pub(in super::super) journal: Option<PersistentControlJournal>,
    pub(in super::super) publisher: ControlSnapshotPublisher,
    pub(in super::super) pin: Option<DirectPin>,
    pub(in super::super) acl_before_open: eliot_searchd::native_file::StorageAclObservation,
}

pub(in super::super) fn journal_owner(
    incarnation: [u8; 16],
    root: [u8; 16],
    epoch: u64,
) -> Result<JournalOwnerBinding, OwnerError> {
    Ok(JournalOwnerBinding::new(
        InstallationIncarnationId::from_bytes(incarnation),
        DataRootId::from_bytes(root),
        OwnerEpoch::new(epoch).map_err(|_| OwnerError::OwnerGuardMismatch)?,
    ))
}
pub(in super::super) fn charge(
    request: &NativeRootRequest,
    work: &mut NativeWork,
    attempt: W,
) -> Result<(), OwnerError> {
    match attempt {
        W::Checkpoint => work.arrive(request, "direct-control-checkpoint"),
        W::Open => {
            work.open(request)?;
            work.call(request, || Ok(()))
        }
        W::Read(n) => {
            work.read(request, n)?;
            work.call(request, || Ok(()))
        }
        W::Write(n) => {
            work.write(request, n)?;
            work.call(request, || Ok(()))
        }
        W::Allocate(n) | W::Process(n) => work.read(request, n),
    }
}
pub(in super::super) fn failure(error: &MigrationBootstrapFailure<OwnerError>) -> OwnerError {
    if error.control == ControlError::CommitOutcomeUnknown {
        OwnerError::OwnerAcquireOutcomeUnknown
    } else {
        error
            .first_context
            .unwrap_or(OwnerError::OwnerRecoveryQuarantined)
    }
}
impl DirectJournal {
    pub(in super::super) fn open(
        request: &NativeRootRequest,
        work: &mut NativeWork,
        root: &Path,
        parent: &File,
        owner: JournalOwnerBinding,
        create: bool,
        storage_acl: &StorageAclSnapshot,
    ) -> Result<Self, OwnerError> {
        let (file, readable) = open_files(request, work, root, parent, create)?;
        // A redb open may mutate. Qualify this actual retained DB handle first.
        let acl_before_open = storage_acl.qualify_file(request, work, &file)?;
        let metadata = work.call(request, || {
            file.metadata().map_err(|_| OwnerError::OwnerGuardMismatch)
        })?;
        super::super::native_objects::require_kind(&metadata, false)?;
        let observed = work.call(request, || {
            eliot_searchd::native_file::observe(&file).map_err(|_| OwnerError::OwnerGuardMismatch)
        })?;
        let native = work.call(request, || native_identity(&file))?;
        if let Some(readable) = readable
            && work.call(request, || native_identity(&readable))? != native
        {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        let identity = crate::control_identity::journal_identity_from_observation(
            owner.installation_incarnation_id(),
            owner.data_root_id(),
            owner.owner_epoch(),
            observed,
        )
        .map_err(|_| OwnerError::OwnerGuardMismatch)?;
        let pin = DirectPin::verified(file, native);
        pin.check(request, work, root)?;
        // The separate pin stays alive throughout redb's ownership of this same object.
        work.open(request)?;
        let db_file = work.call(request, || {
            pin_file(&pin)
                .try_clone()
                .map_err(|_| OwnerError::OwnerGuardMismatch)
        })?;
        let deadline = request.credential_context().0;
        let meter = RefCell::new(work);
        let callback = |attempt| charge(request, &mut meter.borrow_mut(), attempt);
        let mut journal = if create {
            PersistentControlJournal::create_direct_with_original_context(
                db_file,
                identity,
                JournalLimits::BASELINE,
                deadline,
                &callback,
            )
        } else {
            PersistentControlJournal::open_direct_with_original_context(
                db_file,
                identity,
                JournalLimits::BASELINE,
                deadline,
                &callback,
            )
        }
        .map_err(|error| failure(&error))?;
        if create {
            let (id, digest) = request.binding();
            let operation = profile_initialization_operation(id, digest, identity, &callback)?;
            journal
                .initialize_direct_with_original_context(
                    operation,
                    Blake3Digest32::from_bytes(digest),
                    deadline,
                    &callback,
                )
                .map_err(|error| failure(&error))?;
        }
        journal
            .verify_direct_with_original_context(deadline, &callback)
            .map_err(|error| failure(&error))?;
        let mut publisher = ControlSnapshotPublisher::new();
        journal
            .publish_direct_with_original_context(&mut publisher, deadline, &callback)
            .map_err(|error| failure(&error))?;
        pin.check(request, &mut meter.borrow_mut(), root)?;
        Ok(Self {
            journal: Some(journal),
            publisher,
            pin: Some(pin),
            acl_before_open,
        })
    }
    pub(in super::super) fn verify(
        &self,
        request: &NativeRootRequest,
        work: &mut NativeWork,
        root: &Path,
    ) -> Result<(), OwnerError> {
        if let Some(pin) = &self.pin {
            pin.check(request, work, root)?;
        }
        let meter = RefCell::new(work);
        self.journal
            .as_ref()
            .ok_or(OwnerError::OwnerRecoveryQuarantined)?
            .verify_direct_with_original_context(request.credential_context().0, &|attempt| {
                charge(request, &mut meter.borrow_mut(), attempt)
            })
            .map_err(|error| failure(&error))
    }
}

// New private recipe: no historical profile uses its earlier unpublished hash.
// Identity bytes belong to control's existing complete codec, never a path/header.
fn profile_initialization_operation(
    id: search_contracts::RequestId,
    digest: [u8; 32],
    identity: search_control_redb::JournalIdentity,
    work: &dyn Fn(W) -> Result<(), OwnerError>,
) -> Result<MutationId, OwnerError> {
    use search_contracts::{
        BoundedBytes, BoundedList, CanonicalDigestDomain, CanonicalValue, DigestInputLimit,
        blake3_canonical,
    };
    const LIMIT: usize = 256;
    // Bounds include all byte copies, the four-value array, domain preparation
    // and the complete canonical hash preimage, under the ORIGINAL native work.
    work(W::Allocate(
        LIMIT + 4 * std::mem::size_of::<CanonicalValue>(),
    ))?;
    work(W::Process(LIMIT))?;
    let encode = || {
        let domain =
            CanonicalDigestDomain::parse("eliot/cbor/control/direct-profile-initialization/v1")?;
        let value = CanonicalValue::Array(BoundedList::new(vec![
            CanonicalValue::U64(1),
            CanonicalValue::Bytes(BoundedBytes::new(id.as_bytes().to_vec())?),
            CanonicalValue::Bytes(BoundedBytes::new(digest.to_vec())?),
            identity.canonical_binding_value()?,
        ])?);
        blake3_canonical(&domain, &value, DigestInputLimit::new(LIMIT)?)
    };
    encode()
        .map(|value| MutationId(*value.as_bytes()))
        .map_err(|_: search_contracts::ContractError| OwnerError::OwnerGuardMismatch)
}
// Kept within the native owner family; no File/raw handle is exposed to a caller.
const fn pin_file(pin: &DirectPin) -> &File {
    pin.borrow_file()
}

/// Journal and publisher close before root pins and both OS exclusions.
struct DirectRegistryOwner {
    control: DirectJournal,
    owner: Box<NativeRootCapability>,
}
impl DirectRegistryOwner {
    #[cfg(test)]
    pub(crate) fn open(request: NativeRootRequest) -> Result<Self, OwnerError> {
        Self::open_with_work(request, NativeWork::new())
    }
    #[cfg(test)]
    pub(super) fn open_with_work(
        request: NativeRootRequest,
        mut work: NativeWork,
    ) -> Result<Self, OwnerError> {
        let (acquired, control) = acquire_existing_direct(&request, &mut work)?;
        Self::finish_open(request, work, acquired, control)
    }
    pub(super) fn open_for_command(
        request: NativeRootRequest,
        mut work: NativeWork,
        writer: &mut impl Write,
    ) -> Result<Self, String> {
        let (acquired, control) = match acquire_existing_direct(&request, &mut work) {
            Ok(value) => value,
            Err(error) => {
                request
                    .emit_using_work(
                        &mut work,
                        writer,
                        &format!("{{\"error\":\"{}\",\"ready\":false}}", error.code()),
                    )
                    .map_err(|e| e.code().to_owned())?;
                return Err(error.code().to_owned());
            }
        };
        Self::finish_open(request, work, acquired, control).map_err(|e| e.code().to_owned())
    }
    fn finish_open(
        request: NativeRootRequest,
        work: NativeWork,
        acquired: AcquiredNativeRoot,
        control: DirectJournal,
    ) -> Result<Self, OwnerError> {
        let (owner, successor) = acquired.bind(request, work);
        let mut result = Self {
            control,
            owner: Box::new(owner),
        };
        finish_in_place(&mut result.owner, successor)?;
        let mut identity = result
            .control
            .journal
            .as_ref()
            .ok_or(OwnerError::OwnerRecoveryQuarantined)?
            .identity();
        if identity.owner_epoch.get() != result.owner.record.epoch {
            identity.owner_epoch = OwnerEpoch::new(result.owner.record.epoch)
                .map_err(|_| OwnerError::ContractExhausted)?;
            let journal = result
                .control
                .journal
                .take()
                .ok_or(OwnerError::OwnerRecoveryQuarantined)?;
            let owner = &mut result.owner;
            let meter = RefCell::new(&mut owner.work);
            result.control.journal = Some(
                journal
                    .advance_direct_with_original_context(
                        identity,
                        owner.request.credential_context().0,
                        &|attempt| charge(&owner.request, &mut meter.borrow_mut(), attempt),
                    )
                    .map_err(|error| failure(&error))?,
            );
        }
        result.owner.verify()?;
        result.control.verify(
            &result.owner.request,
            &mut result.owner.work,
            &result.owner.root,
        )?;
        Ok(result)
    }
    /// Transfers this exact DIRECT owner without reopening or advancing it.
    #[allow(dead_code)] // Dormant until the qualified provider composition calls it.
    pub(in super::super) fn into_process_custody(
        mut self,
        pin_limits: search_retention::sweep::SweepLimits,
    ) -> Result<NativeProcessRootCustody, OwnerError> {
        self.owner.verify()?;
        self.control.verify(&self.owner.request, &mut self.owner.work, &self.owner.root)?;
        let identity = self.control.journal.as_ref()
            .ok_or(OwnerError::OwnerRecoveryQuarantined)?.identity();
        if identity.owner_epoch.get() != self.owner.record.epoch {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        let source_pins = search_retention::source_pins::SourcePinRegistry::new(pin_limits)
            .map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
        Ok(NativeProcessRootCustody { owner: self, journal_identity: identity, source_pins })
    }
    pub(crate) fn execute(
        mut self,
        hints: &crate::config_composition::RootLocatorHints,
        selected: Option<&Path>,
        policy: Option<&super::super::direct_policy_v2::CapturedPolicyV2>,
        source: Option<&source_command::NativeSourceCommand>,
        writer: &mut impl Write,
    ) -> Result<(), String> {
        let result = if let Some(source) = source {
            self.prepare_source_output(hints, selected, policy, source)
        } else { self.prepare_output(hints, selected, policy) };
        self.complete_output(result, writer)
    }
    fn complete_output(mut self, result: Result<command::DirectOutput, String>, writer: &mut impl Write) -> Result<(), String> {
        match result {
            Ok(output) => {
                self.owner
                    .emit(writer, &output.line)
                    .map_err(|e| e.code().to_owned())?;
                self.finish().map_err(|e| e.code().to_owned())?;
                if output.current {
                    Ok(())
                } else {
                    Err("DIRECT_ROOT_CURRENT_QUALIFICATION_REFUSED".into())
                }
            }
            Err(error) => {
                let refusal = if self.owner.effect_observed {
                    OwnerError::OwnerAcquireOutcomeUnknown
                } else {
                    OwnerError::OwnerRecoveryQuarantined
                };
                self.owner.poisoned.get_or_insert(refusal);
                Err(error)
            }
        }
    }
    pub(super) fn finish(mut self) -> Result<(), OwnerError> {
        // Read back only the retained exact journal. FULL gates remain before
        // each lifecycle write and after readback/output, before custody drops.
        self.control
            .verify(&self.owner.request, &mut self.owner.work, &self.owner.root)?;
        if self.owner.mutation {
            self.owner.begin_drain(DrainReason::Shutdown)?;
            self.owner.release_cleanly()?;
        }
        self.owner.verify()?;
        let Self { control, owner } = self;
        drop(control);
        let NativeRootCapability {
            pins,
            root,
            installation,
            record,
            request,
            mut work,
            bound_root,
            exclusions,
            ..
        } = *owner;
        drop((pins, root, installation, record, bound_root));
        let closed = work.arrive(&request, "direct-children-closed");
        drop(exclusions);
        let released = work.arrive(&request, "direct-custody-released");
        closed
            .and(released)
            .map_err(|_| OwnerError::OwnerReleaseOutcomeUnknown)
    }
}

#[path = "direct_registry_command.rs"]
mod command;
#[path = "native_source_command.rs"]
mod source_command;
#[path = "native_retention_command.rs"]
mod retention_command;
#[path = "native_retention_control.rs"]
mod retention_control;
#[path = "native_retention_source.rs"]
mod retention_source;
#[path = "native_retention.rs"]
mod retention;
#[cfg(feature = "wave3-index")]
#[path = "native_artifact_store.rs"]
pub(in super::super) mod artifact_store;
#[cfg(feature = "wave3-index")]
#[path = "native_artifact_reader.rs"]
pub(in super::super) mod artifact_reader;
#[path = "native_retained_reader.rs"]
mod retained_reader;
#[path = "native_dek_command.rs"]
mod dek_command;
#[path = "native_dek_selection.rs"]
mod dek_selection;
#[path = "direct_registry_process_custody.rs"]
mod process_custody;
pub(crate) use process_custody::NativeProcessRootCustody;
#[cfg(feature = "wave4-query")]
#[path = "native_query_population.rs"]
mod query_population;
#[cfg(feature = "wave4-query")]
pub(crate) use query_population::NativePopulationInput;
#[cfg(feature = "wave4-query")]
pub(crate) use retained_reader::RetainedRevisionReadState;
pub use command::maybe_run;

#[cfg(all(test, windows))]
#[path = "direct_registry_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "direct_profile_initialization_tests.rs"]
mod profile_tests;

// Physical opening is closed over one retained parent and one fixed leaf.
fn open_files(
    request: &NativeRootRequest,
    work: &mut NativeWork,
    root: &Path,
    parent: &File,
    create: bool,
) -> Result<(File, Option<File>), OwnerError> {
    let path = root.join("control/control.redb");
    // Fresh absolute parent READ/traverse, then bind creation to the actual
    // retained control FD. No possible create is dispatched by path.
    let current_parent = super::super::native_objects::open_existing(
        request,
        work,
        &root.join("control"),
        true,
        false,
    )?;
    if work.call(request, || native_identity(&current_parent))?
        != work.call(request, || native_identity(parent))?
    {
        return Err(OwnerError::OwnerGuardMismatch);
    }
    drop(current_parent);
    // Existing-file access semantics remain fresh full-path READ/traverse.
    // Compare that opened object with the parent-bound RW FD BEFORE redb I/O.
    let readable = if create {
        None
    } else {
        Some(super::super::native_objects::open_existing(
            request, work, &path, false, false,
        )?)
    };
    work.open(request)?;
    let file = work.call(request, || {
        super::super::registration_native::open_direct_control(parent, create).map_err(|error| {
            if create {
                OwnerError::OwnerAcquireOutcomeUnknown
            } else {
                error
            }
        })
    })?;
    Ok((file, readable))
}
