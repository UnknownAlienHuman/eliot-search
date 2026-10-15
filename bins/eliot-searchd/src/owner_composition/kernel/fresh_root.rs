//! Fresh root composition under the original native request. No legacy upgrade.
use super::NativeReadOperation;
use eliot_searchd::native_file::{Observation, ObservationError, QualifiedDirectory};
use search_contracts::{self as c, CanonicalValue as V};
use search_ports::{CancellationProbe, OperationContext, PackageOpaque};
use search_revision_store::LegacyRevisionObjectWork as Work;
use search_runtime_owner::OwnerError;
use search_source_admission::{self as admission, ValidatedAdmissionConfig};
use search_source_identity::{
    ntfs_directory,
    observation_codec::{binary, tuple},
};
use search_source_registry::{
    api::{SourceRegistry, register_root},
    error::{RegistryControlPort, RegistryError},
    root::{RegisterRootRequest, RootRecord},
};
use std::{
    cell::RefCell,
    fs::File,
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
    time::Instant,
};

pub const OWNER_FENCE_DOMAIN: &str = "eliot/cbor/source-registry/root-owner-fence/v1";
pub const ROOT_POLICY_DOMAIN: &str = "eliot/cbor/source-registry/root-bootstrap-policy/v1";

#[path = "fresh_root_policy_v2.rs"]
mod policy_v2;
use policy_v2::RootPolicy;

#[derive(Debug)]
pub enum FreshRootError {
    Native(OwnerError),
    Admission(admission::AdmissionError),
    Registry(RegistryError),
}
impl std::fmt::Display for FreshRootError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Native(error) => write!(f, "{error:?}"),
            Self::Admission(error) => write!(f, "{error:?}"),
            Self::Registry(error) => write!(f, "{error:?}"),
        }
    }
}
const fn invalid() -> FreshRootError {
    FreshRootError::Registry(RegistryError::SnapshotInvalid)
}
fn array(v: Vec<V>) -> Result<V, FreshRootError> {
    tuple(v).map_err(|_| invalid())
}
fn bytes(v: &[u8]) -> Result<V, FreshRootError> {
    binary(v).map_err(|_| invalid())
}
fn text(v: &str) -> Result<V, FreshRootError> {
    c::CanonicalText::new(v).map(V::Text).map_err(|_| invalid())
}
fn hash(domain: &str, v: &V) -> Result<c::Blake3Digest32, FreshRootError> {
    c::blake3_canonical(
        &c::CanonicalDigestDomain::parse(domain).map_err(|_| invalid())?,
        v,
        c::DigestInputLimit::new(admission::original_envelope::MAX_ORIGINAL_ADMISSION_BYTES)
            .map_err(|_| invalid())?,
    )
    .map_err(|_| invalid())
}
fn reference(digest: c::Blake3Digest32) -> Result<c::ReceiptRef, FreshRootError> {
    c::ReceiptRef::new(format!("blake3:{digest}")).map_err(|_| invalid())
}
fn receipt_digests(
    owner: &V,
    request: &V,
    native: V,
    intent: V,
    config: V,
    policy: V,
) -> Result<(c::Blake3Digest32, c::Blake3Digest32), FreshRootError> {
    let fence = array(vec![
        V::U64(1),
        owner.clone(),
        request.clone(),
        native.clone(),
        intent.clone(),
    ])?;
    let policy = array(vec![
        V::U64(1),
        owner.clone(),
        request.clone(),
        native,
        intent,
        config,
        policy,
    ])?;
    Ok((
        hash(OWNER_FENCE_DOMAIN, &fence)?,
        hash(ROOT_POLICY_DOMAIN, &policy)?,
    ))
}

/// Same original flag and absolute deadline; constructing a port context renews neither.
#[derive(Debug)]
pub struct FreshRootCancellation {
    flag: Arc<AtomicBool>,
    deadline: Instant,
    first_refusal: AtomicU8,
}
impl FreshRootCancellation {
    fn refusal(&self) -> Option<OwnerError> {
        let observed = if self.flag.load(Ordering::Acquire) {
            1
        } else if Instant::now() >= self.deadline {
            2
        } else {
            0
        };
        if observed != 0 {
            let _ = self.first_refusal.compare_exchange(
                0,
                observed,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }
        match self.first_refusal.load(Ordering::Acquire) {
            0 => None,
            1 => Some(OwnerError::OwnerCancelledBeforeMutation),
            _ => Some(OwnerError::OwnerDeadlineInvalid),
        }
    }
}
impl PackageOpaque for FreshRootCancellation {
    fn owner_package(&self) -> &'static str {
        "eliot-searchd"
    }
}
impl CancellationProbe for FreshRootCancellation {
    fn is_cancelled(&self) -> bool {
        self.refusal().is_some()
    }
}

/// Borrowed original native custody plus its exact O/Q values. No public constructor.
pub struct FreshRootContext<'operation, 'owner> {
    operation: RefCell<&'operation mut NativeReadOperation<'owner>>,
    context: OperationContext<FreshRootCancellation>,
    owner: V,
    request: V,
    epoch: c::OwnerEpoch,
}
impl<'owner> NativeReadOperation<'owner> {
    pub(crate) fn fresh_root_context(
        &mut self,
    ) -> Result<FreshRootContext<'_, 'owner>, FreshRootError> {
        self.verify().map_err(FreshRootError::Native)?;
        let (body, record_digest, id, digest, epoch, context) = self
            .with_binding(|binding, request, work, _| {
                // Bound processing/allocation before copying the complete existing codec.
                work.read(request, 16 * 1024)?;
                let body = binding.record.encode_body();
                let (id, digest) = request.binding();
                let deadline = request.credential_context().0;
                let remaining = u64::try_from(
                    deadline
                        .saturating_duration_since(Instant::now())
                        .as_millis(),
                )
                .map_err(|_| OwnerError::ContractExhausted)?;
                let context = OperationContext::new(
                    id,
                    remaining,
                    FreshRootCancellation {
                        flag: request.original_cancellation(),
                        deadline,
                        first_refusal: AtomicU8::new(0),
                    },
                    c::OpaqueRef::new("original-native-root-budget")
                        .map_err(|_| OwnerError::ContractExhausted)?,
                )
                .map_err(|_| OwnerError::OwnerOperationConflict)?;
                Ok((
                    body,
                    binding.record.record_digest,
                    id,
                    digest,
                    binding.record.epoch,
                    context,
                ))
            })
            .map_err(FreshRootError::Native)?;
        let owner = array(vec![bytes(&body)?, bytes(&record_digest)?])?;
        let request = array(vec![bytes(id.as_bytes())?, bytes(&digest)?])?;
        let epoch = c::OwnerEpoch::new(epoch).map_err(|_| invalid())?;
        Ok(FreshRootContext {
            operation: RefCell::new(self),
            context,
            owner,
            request,
            epoch,
        })
    }
}

struct Pin {
    path: PathBuf,
    directory: QualifiedDirectory,
    observed: Observation,
}
/// Retained native source-directory selection and deterministic registration payload.
/// Absence of a `RootRecord` proof prevents obtaining this through a legacy locator.
pub struct PreparedFreshRoot<'view, 'operation, 'owner> {
    native: &'view FreshRootContext<'operation, 'owner>,
    pins: Vec<Pin>,
    request: RegisterRootRequest,
    policy: RootPolicy,
    operation: c::OpaqueId,
    expected: u64,
    mutation: c::Blake3Digest32,
}
/// Full `RootRecord` readback plus retained native selection; no timeless access grant.
pub struct RegisteredFreshRoot<'prepared, 'view, 'operation, 'owner> {
    prepared: &'prepared PreparedFreshRoot<'view, 'operation, 'owner>,
    root: RootRecord,
}

/// Existing metadata joined to freshly observed Native custody and an explicit policy.
/// Construction issues no new registration or original-operation receipt.
pub struct RequalifiedFreshRoot<'view, 'operation, 'owner> {
    native: &'view FreshRootContext<'operation, 'owner>, pins: Vec<Pin>, root: RootRecord,
    policy: RootPolicy,
}

impl<'operation, 'owner> FreshRootContext<'operation, 'owner> {
    // A port can observe original cancellation before the next Native seam.
    // Carry that first refusal into the original owner even if the external
    // flag is subsequently reset; no replacement context or allowance exists.
    fn guard(&self) -> Result<(), OwnerError> {
        if let Some(error) = self.context.cancellation().refusal() {
            return self
                .operation
                .borrow_mut()
                .with_binding(|_, _, _, _| Err(error));
        }
        Ok(())
    }
    pub const fn context(&self) -> &OperationContext<FreshRootCancellation> {
        &self.context
    }
    pub fn work(
        &self,
        work: search_control_redb::MigrationBootstrapWork,
    ) -> Result<(), OwnerError> {
        use search_control_redb::MigrationBootstrapWork as W;
        self.guard()?;
        let mut operation = self
            .operation
            .try_borrow_mut()
            .map_err(|_| OwnerError::OwnerOperationConflict)?;
        match work {
            W::Checkpoint => operation.migration_checkpoint(),
            W::Open => operation.migration_charge(Work::Open),
            W::Read(n) => operation.migration_charge(Work::Read(n)),
            W::Write(n) => operation.migration_charge(Work::Write(n)),
            W::Allocate(n) => operation.migration_charge(Work::Allocate(n)),
            W::Process(n) => operation.migration_charge(Work::Compare(n)),
        }
    }
    fn checkpoint(&self) -> Result<(), FreshRootError> {
        self.guard().map_err(FreshRootError::Native)?;
        self.operation
            .borrow_mut()
            .migration_checkpoint()
            .map_err(FreshRootError::Native)
    }
    fn verify_owner(&self) -> Result<(), FreshRootError> {
        self.guard().map_err(FreshRootError::Native)?;
        self.operation
            .borrow_mut()
            .verify()
            .map_err(FreshRootError::Native)
    }
    fn charge(&self, work: Work) -> Result<(), FreshRootError> {
        self.guard().map_err(FreshRootError::Native)?;
        self.operation
            .borrow_mut()
            .migration_charge(work)
            .map_err(FreshRootError::Native)
    }
    fn observe(
        &self,
        file: Arc<File>,
    ) -> Result<(QualifiedDirectory, Observation), FreshRootError> {
        self.checkpoint()?;
        let first = RefCell::new(None);
        let check = |before| {
            let mut op = self.operation.borrow_mut();
            let result = if before {
                op.migration_held_observer_dispatch()
            } else {
                op.migration_checkpoint()
            };
            result.map_err(|error| {
                if first.borrow().is_none() {
                    *first.borrow_mut() = Some(error);
                }
                ObservationError::ObservationFailed
            })
        };
        let observed = QualifiedDirectory::qualify_with_check(file, &check);
        let after = self.checkpoint();
        if let Some(error) = first.into_inner() {
            return Err(FreshRootError::Native(error));
        }
        after?;
        observed.map_err(|_| invalid())?.ok_or_else(invalid)
    }
    fn open(&self, path: &Path) -> Result<(QualifiedDirectory, Observation), FreshRootError> {
        self.charge(Work::Open)?;
        #[cfg(windows)]
        let opened = {
            use std::os::windows::fs::OpenOptionsExt;
            // Current READ/LIST plus explicit traverse; no write, execution or reparse following.
            std::fs::OpenOptions::new()
                .access_mode(0x0012_00a9)
                .share_mode(7)
                .custom_flags(0x0220_0000)
                .open(path)
        };
        #[cfg(not(windows))]
        let opened: std::io::Result<File> = Err(std::io::ErrorKind::Unsupported.into());
        self.checkpoint()?;
        let file = opened.map_err(|_| invalid())?;
        self.charge(Work::Allocate(
            std::mem::size_of::<File>() + 2 * std::mem::size_of::<usize>(),
        ))?;
        self.observe(Arc::new(file))
    }

    fn selection(&self, source_root: &Path) -> Result<(Vec<Pin>, c::Blake3Digest32), FreshRootError> {
        if !source_root.is_absolute() || source_root.as_os_str().len() > 2048 { return Err(invalid()); }
        self.charge(Work::AllocateLocator(16 * 1024))?;
        let mut pins = Vec::with_capacity(8);
        let mut path = PathBuf::new();
        for component in source_root.components() {
            match component {
                Component::Prefix(_) => { path.push(component); continue; }
                Component::RootDir | Component::Normal(_) => path.push(component),
                _ => return Err(invalid()),
            }
            if pins.len() == 8 { return Err(FreshRootError::Registry(RegistryError::CapacityExceeded)); }
            let (directory, observed) = self.open(&path)?;
            pins.push(Pin { path: path.clone(), directory, observed });
        }
        let observed = pins.last().ok_or_else(invalid)?.observed;
        let digest = ntfs_directory::ntfs_directory_digest(observed.volume_serial, observed.file_index, observed.creation_time)
            .map_err(|_| invalid())?;
        Ok((pins, digest))
    }
    pub fn requalify_existing<'view>(&'view self, path: &Path, id: c::RootBindingId,
        registry: &SourceRegistry, config: &ValidatedAdmissionConfig,
    ) -> Result<RequalifiedFreshRoot<'view, 'operation, 'owner>, FreshRootError> {
        let policy = admission::compile_bootstrap_policy_v1(config).map_err(FreshRootError::Admission)?;
        self.requalify_policy(path, id, registry, RootPolicy::V1(policy))
    }

    fn requalify_policy<'view>(&'view self, path: &Path, id: c::RootBindingId,
        registry: &SourceRegistry, policy: RootPolicy,
    ) -> Result<RequalifiedFreshRoot<'view, 'operation, 'owner>, FreshRootError> {
        self.verify_owner()?;
        let root = registry.roots().get(&id).ok_or_else(invalid)?.clone();
        let (pins, digest) = self.selection(path)?;
        if root.status != search_source_registry::root::RootStatus::Active || root.canonical_root_digest != digest
            || root.policy_revision != policy.revision()
            || root.policy_fingerprint != policy.fingerprint() { return Err(invalid()); }
        let result = RequalifiedFreshRoot { native: self, pins, root, policy };
        result.check_current(registry)?;
        Ok(result)
    }

    /// Join only DB-selected Root keys; config contributes locators, never membership.
    pub(crate) fn requalify_roots(&self, registry: &SourceRegistry,
        hints: &crate::config_composition::RootLocatorHints, config: &ValidatedAdmissionConfig,
    ) -> Result<(), FreshRootError> {
        for id in registry.roots().keys() {
            let qualified = self.requalify_existing(hints.get(id).map_err(|_| invalid())?, *id, registry, config)?;
            if qualified.root().root_binding_id != *id { return Err(invalid()); }
        }
        Ok(())
    }

    pub fn prepare<'view>(
        &'view self,
        source_root: &Path,
        root_id: c::RootBindingId,
        operation: c::OpaqueId,
        expected_registry_revision: u64,
        config: &ValidatedAdmissionConfig,
    ) -> Result<PreparedFreshRoot<'view, 'operation, 'owner>, FreshRootError> {
        // All five owner-validated fields checked before native source I/O.
        let policy =
            admission::compile_bootstrap_policy_v1(config).map_err(FreshRootError::Admission)?;
        let (config_value, policy_value) =
            admission::bootstrap_v1::bootstrap_policy_evidence_v1(config)
                .map_err(FreshRootError::Admission)?;
        self.prepare_policy(source_root, root_id, operation, expected_registry_revision,
            RootPolicy::V1(policy), config_value, policy_value)
    }

    #[expect(clippy::too_many_arguments, reason = "One existing Root preparation; policy evidence stays paired at its two closed callers")]
    fn prepare_policy<'view>(
        &'view self, source_root: &Path, root_id: c::RootBindingId, operation: c::OpaqueId,
        expected_registry_revision: u64, policy: RootPolicy, config_value: V, policy_value: V,
    ) -> Result<PreparedFreshRoot<'view, 'operation, 'owner>, FreshRootError> {
        let (pins, canonical_root_digest) = self.selection(source_root)?;
        let observed = pins.last().ok_or_else(invalid)?.observed;
        let fingerprint = policy.fingerprint();
        let mut intent_fields = vec![
            bytes(root_id.as_bytes())?,
            bytes(canonical_root_digest.as_bytes())?,
            bytes(fingerprint.as_bytes())?,
            V::U64(policy.revision().get()),
            V::U64(self.epoch.get()),
            text(operation.as_str())?,
            V::U64(expected_registry_revision),
        ];
        if let RootPolicy::V2 { reference, .. } = &policy {
            intent_fields.push(bytes(reference)?);
        }
        let intent = array(intent_fields)?;
        let native = array(vec![
            text(ntfs_directory::NTFS_DIRECTORY_PROFILE)?,
            V::U64(u64::from(observed.volume_serial)),
            V::U64(observed.file_index),
            V::U64(u64::from(observed.attributes)),
            V::U64(observed.length),
            V::U64(observed.creation_time),
            V::U64(observed.last_write_time),
        ])?;
        self.charge(Work::Allocate(32 * 1024))?;
        self.charge(Work::Compare(32 * 1024))?;
        let (mutation, policy_digest) = policy.receipt_digests(
            &self.owner,
            &self.request,
            native,
            intent,
            config_value,
            policy_value,
        )?;
        let request = RegisterRootRequest {
            root_binding_id: root_id,
            canonical_root_digest,
            policy_fingerprint: fingerprint,
            policy_revision: policy.revision(),
            owner_epoch: self.epoch,
            owner_fence_receipt: reference(mutation)?,
            policy_receipt: reference(policy_digest)?,
        };
        let prepared = PreparedFreshRoot {
            native: self,
            pins,
            request,
            policy,
            operation,
            expected: expected_registry_revision,
            mutation,
        };
        prepared.check_current()?;
        Ok(prepared)
    }
}

#[cfg(test)]
mod golden_tests {
    use super::super::super::super::{
        codec::blake3_bytes,
        record::DurableOwnerRecord,
        spec::{DrainReasonText, LifecycleState},
    };
    use super::*;
    #[test]
    fn fresh_root_receipt_canonical_goldens_bind_all_fields() {
        // Deterministic owner-codec fixture, never a Native capability.
        let mut record = DurableOwnerRecord {
            installation_id: [1; 16],
            installation_incarnation_id: [2; 16],
            data_root_id: [3; 16],
            epoch: 1,
            previous_epoch: 0,
            previous_record_digest: [0; 32],
            canonical_path_digest: [4; 32],
            volume_identity_digest: [5; 32],
            executable_digest: [6; 32],
            owner_token: [7; 16],
            owner_pid: 42,
            lifecycle: LifecycleState::Released,
            drain_reason: DrainReasonText::Shutdown,
            generation: 2,
            record_digest: [0; 32],
        };
        let body = record.encode_body();
        record.record_digest = blake3_bytes(&body);
        let owner = array(vec![
            bytes(&body).unwrap(),
            bytes(&record.record_digest).unwrap(),
        ])
        .unwrap();
        let request = array(vec![bytes(&[8; 16]).unwrap(), bytes(&[9; 32]).unwrap()]).unwrap();
        let section = admission::validate_section(
            &admission::compiled_defaults(),
            &admission::SectionPlatform {
                os: "windows".into(),
                arch: "x64".into(),
            },
            &admission::AcceptedCapabilities {
                profiles: vec![admission::BASELINE_PROFILE.into()],
            },
        )
        .unwrap();
        let policy = admission::compile_bootstrap_policy_v1(&section).unwrap();
        let (config, policy) = admission::bootstrap_v1::bootstrap_policy_evidence_v1(&section)
            .map(|(c, p)| ((c, admission::policy_fingerprint(&policy)), p))
            .unwrap();
        let physical =
            ntfs_directory::ntfs_directory_digest(u32::MAX, u64::MAX, 133_700_000_000_000_000)
                .unwrap();
        let native = array(vec![
            text(ntfs_directory::NTFS_DIRECTORY_PROFILE).unwrap(),
            V::U64(u64::from(u32::MAX)),
            V::U64(u64::MAX),
            V::U64(0x10),
            V::U64(0),
            V::U64(133_700_000_000_000_000),
            V::U64(133_700_000_000_000_001),
        ])
        .unwrap();
        let intent = array(vec![
            bytes(&[10; 16]).unwrap(),
            bytes(physical.as_bytes()).unwrap(),
            bytes(config.1.as_bytes()).unwrap(),
            V::U64(1),
            V::U64(1),
            text("golden-fresh-root-O1").unwrap(),
            V::U64(0),
        ])
        .unwrap();
        let digests = receipt_digests(
            &owner,
            &request,
            native.clone(),
            intent.clone(),
            config.0.clone(),
            policy.clone(),
        )
        .unwrap();
        assert_eq!(
            digests.0.to_string(),
            "01ed340fbee18deb82614293e483cc080dbe61d77d962838ecad2ab688184733"
        );
        assert_eq!(
            digests.1.to_string(),
            "ce573852665102375485ee8c2642f584f5b56e25263fabfb63c7fab1e105bd93"
        );
        // Every O/Q/N/I/C/P change affects the appropriate reference. Samples
        // here are adversarial CBOR mutations, not valid authority observations.
        for field in 0..6 {
            let mut values = [
                owner.clone(),
                request.clone(),
                native.clone(),
                intent.clone(),
                config.0.clone(),
                policy.clone(),
            ];
            values[field] = V::Null;
            let [o, q, n, i, c, p] = values;
            let changed = receipt_digests(&o, &q, n, i, c, p).unwrap();
            assert_ne!(changed.1, digests.1);
            if field < 4 {
                assert_ne!(changed.0, digests.0);
            }
        }
    }
}

impl<'view, 'operation, 'owner> PreparedFreshRoot<'view, 'operation, 'owner> {
    fn check_current(&self) -> Result<(), FreshRootError> {
        self.native.checkpoint()?;
        for pin in &self.pins {
            let (_, held) = self.native.observe(pin.directory.retain_file())?;
            let (_, current) = self.native.open(&pin.path)?;
            let same = |value: Observation| {
                value.volume_serial == pin.observed.volume_serial
                    && value.file_index == pin.observed.file_index
                    && value.creation_time == pin.observed.creation_time
            };
            if !same(held) || !same(current) {
                return Err(invalid());
            }
        }
        self.native.checkpoint()
    }
    pub fn register<'prepared, P: RegistryControlPort<Cancellation = FreshRootCancellation>>(
        &'prepared self,
        registry: &mut SourceRegistry,
        port: &mut P,
    ) -> Result<RegisteredFreshRoot<'prepared, 'view, 'operation, 'owner>, FreshRootError> {
        self.native.verify_owner()?;
        self.check_current()?;
        match port
            .load_batch(&self.operation, self.native.context())
            .map_err(|_| FreshRootError::Registry(RegistryError::MutationOutcomeUnknown))?
        {
            search_source_registry::recovery::RegistryBatchLookup::Absent { revision }
                if revision != self.expected || registry.revision() != self.expected =>
            {
                return Err(FreshRootError::Registry(
                    RegistryError::RegistryRevisionConflict,
                ));
            }
            _ => {}
        }
        let receipt = self.request.owner_fence_receipt.clone();
        register_root(
            registry,
            &self.request,
            &self.operation,
            self.mutation,
            &receipt,
            port,
            self.native.context(),
        )
        .map_err(FreshRootError::Registry)?;
        // Owner API already rechecks immutable operation and full target readback;
        // compare the actual installed current RootRecord before admission escapes.
        let root = registry
            .roots()
            .get(&self.request.root_binding_id)
            .ok_or_else(invalid)?
            .clone();
        if root.canonical_root_digest != self.request.canonical_root_digest
            || root.policy_fingerprint != self.request.policy_fingerprint
            || root.policy_revision != self.request.policy_revision
            || root.owner_epoch != self.request.owner_epoch
            || root.record_revision.get() != 1
            || Some(root.registry_revision) != self.expected.checked_add(1)
            || root.status != search_source_registry::root::RootStatus::Active
            || root.last_receipt != receipt
        {
            return Err(invalid());
        }
        self.check_current()
            .map_err(|_| FreshRootError::Registry(RegistryError::MutationOutcomeUnknown))?;
        self.native
            .verify_owner()
            .map_err(|_| FreshRootError::Registry(RegistryError::MutationOutcomeUnknown))?;
        Ok(RegisteredFreshRoot {
            prepared: self,
            root,
        })
    }
}
impl RegisteredFreshRoot<'_, '_, '_, '_> {
    pub const fn root(&self) -> &RootRecord {
        &self.root
    }
    pub fn policy_revision(&self) -> c::NonZeroRevision {
        self.prepared.policy.revision()
    }
}

impl RequalifiedFreshRoot<'_, '_, '_> {
    pub const fn root(&self) -> &RootRecord { &self.root }
    pub fn check_current(&self, registry: &SourceRegistry) -> Result<(), FreshRootError> {
        self.native.verify_owner()?;
        if registry.roots().get(&self.root.root_binding_id) != Some(&self.root)
            || self.policy.fingerprint() != self.root.policy_fingerprint { return Err(invalid()); }
        for pin in &self.pins {
            let (_, held) = self.native.observe(pin.directory.retain_file())?;
            let (_, current) = self.native.open(&pin.path)?;
            for value in [held, current] {
                if value.volume_serial != pin.observed.volume_serial || value.file_index != pin.observed.file_index
                    || value.creation_time != pin.observed.creation_time { return Err(invalid()); }
            }
        }
        self.native.checkpoint()
    }
}
