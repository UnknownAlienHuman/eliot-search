//! Explicit empty-root initialization and exact, non-replaying finalization.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use search_contracts::{
    BoundedBytes, BoundedList, CanonicalDigestDomain, CanonicalValue, DigestInputLimit,
    blake3_canonical,
};
use search_runtime_owner::{DrainReason, OwnerError};

use super::codec::{hex, parse_digest32, parse_id16, parse_u32, sync_directory};
use super::installation::{
    InstallationBinding, load_existing_installation, publish_initialized_installation,
};
use super::lifecycle::LiveOwner;
use super::observation::{
    mint_installation_ids, mint_owner_token, observe_executable, observe_physical_root,
};
use super::operation::DataRootRequest;
use super::read_existing::{
    open_bound_directory, read_existing_bytes, verify_bound_directory, verify_existing_locator,
};
use super::record::DurableOwnerRecord;
use super::slots::newest_valid;
use super::spec::{INSTALLATION_FILE, LifecycleState, OWNER_SLOT_A, OWNER_SLOT_B};

const INTENT: &str = ".eliot-search-initialization-intent.v1";
const PRIMARY: &str = ".eliot-search-owner.lock";
const SEALED: &str = ".eliot-search-sealed-owner.lock";
const MAX_INTENT: usize = 2048;

/// A requested operation name, never root or recovery authority by itself.
pub(crate) struct InitializationRequest([u8; 16]);

impl InitializationRequest {
    pub(crate) fn parse(value: &str) -> Result<Self, OwnerError> {
        let id = parse_id16(Some(value))?;
        if id == [0; 16] {
            return Err(OwnerError::OwnerOperationConflict);
        }
        Ok(Self(id))
    }
}

#[derive(Clone, Eq, PartialEq)]
struct InitIntent {
    id: [u8; 16],
    installation: [u8; 16],
    incarnation: [u8; 16],
    root: [u8; 16],
    executable: [u8; 32],
    namespace: [u8; 32],
    owner_token: [u8; 16],
    owner_pid: u32,
}

impl InitIntent {
    fn encode(&self) -> Vec<u8> {
        format!(
            "ELIOT-SEARCH-INITIALIZATION-V1\nformat_version=1\noperation_id={}\ninstallation_id={}\ninstallation_incarnation_id={}\ndata_root_id={}\nexecutable_digest={}\nnamespace_id={}\nowner_token={}\nowner_pid={}\n",
            hex(&self.id), hex(&self.installation), hex(&self.incarnation), hex(&self.root),
            hex(&self.executable), hex(&self.namespace), hex(&self.owner_token), self.owner_pid,
        ).into_bytes()
    }

    fn decode(bytes: &[u8]) -> Result<Self, OwnerError> {
        let text = std::str::from_utf8(bytes).map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
        let lines = text.lines().collect::<Vec<_>>();
        if bytes.len() > MAX_INTENT
            || lines.len() != 10
            || lines[0] != "ELIOT-SEARCH-INITIALIZATION-V1"
            || lines[1] != "format_version=1"
        {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
        let intent = Self {
            id: parse_id16(lines[2].strip_prefix("operation_id="))?,
            installation: parse_id16(lines[3].strip_prefix("installation_id="))?,
            incarnation: parse_id16(lines[4].strip_prefix("installation_incarnation_id="))?,
            root: parse_id16(lines[5].strip_prefix("data_root_id="))?,
            executable: parse_digest32(lines[6].strip_prefix("executable_digest="))?,
            namespace: parse_digest32(lines[7].strip_prefix("namespace_id="))?,
            owner_token: parse_id16(lines[8].strip_prefix("owner_token="))?,
            owner_pid: parse_u32(lines[9].strip_prefix("owner_pid="))?,
        };
        if intent.id == [0; 16]
            || intent.installation == [0; 16]
            || intent.incarnation == [0; 16]
            || intent.root == [0; 16]
            || intent.namespace == [0; 32]
            || intent.owner_token == [0; 16]
            || intent.owner_pid == 0
            || intent.encode() != bytes
        {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
        Ok(intent)
    }

    fn verify_root(&self, root: &Path) -> Result<(), OwnerError> {
        if observe_physical_root(root)?.data_root_id.as_bytes() != &self.root
            || observe_executable()? != self.executable
        {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        Ok(())
    }
}

/// Private initialization authority. Only the empty-root proof plus retained
/// native exclusions and exact intent can construct it; no ordinary open can.
pub(crate) struct InitializingDataRoot {
    root: PathBuf,
    intent: InitIntent,
    root_file: File,
    primary: File,
    sealed: Option<crate::sealed_root_lock::SealedRootLease>,
    intent_file: File,
    operation: DataRootRequest,
}

impl InitializingDataRoot {
    pub(crate) fn operation_request(&self) -> Result<&DataRootRequest, String> {
        self.verify().map_err(code)?;
        Ok(&self.operation)
    }

    pub(crate) fn canonical_root(&self) -> &Path {
        &self.root
    }
    pub(crate) const fn namespace_id(&self) -> [u8; 32] {
        self.intent.namespace
    }

    pub(crate) fn verify(&self) -> Result<(), OwnerError> {
        // The durable intent already exists. An expired/cancelled invocation
        // cannot certify a no-effect outcome or discard those retained inputs.
        self.operation
            .check()
            .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)?;
        verify_bound_directory(&self.root_file, &self.root)?;
        verify_existing_locator(&self.primary, &self.root.join(PRIMARY))?;
        verify_existing_locator(&self.intent_file, &self.root.join(INTENT))?;
        if let Some(sealed) = &self.sealed {
            sealed
                .verify_existing(&self.root)
                .map_err(|_| OwnerError::OwnerGuardMismatch)?;
        }
        self.intent.verify_root(&self.root)?;
        let actual = read_existing_bytes(&self.root.join(INTENT), MAX_INTENT)?
            .ok_or(OwnerError::OwnerAcquireOutcomeUnknown)?;
        if actual != self.intent.encode() {
            return Err(OwnerError::OwnerOperationConflict);
        }
        self.operation
            .check()
            .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)?;
        Ok(())
    }
}

/// Named initialization recovery exposes existing readback/finalization only.
/// It cannot be passed to the layout/credential creation child.
pub(crate) struct InitializationRecovery<'a> {
    root: &'a InitializingDataRoot,
}

impl InitializationRecovery<'_> {
    pub(crate) fn operation_request(&self) -> Result<&DataRootRequest, String> {
        self.root.operation_request()
    }

    pub(crate) fn canonical_root(&self) -> &Path {
        self.root.canonical_root()
    }
    pub(crate) fn namespace_id(&self) -> [u8; 32] {
        self.root.namespace_id()
    }
    pub(crate) fn verify(&self) -> Result<(), OwnerError> {
        self.root.verify()
    }
}

/// Exact completion readback; does not grant ordinary read/write access.
pub(crate) struct InitializationReceipt {
    pub(crate) namespace: [u8; 32],
    pub(crate) replayed: bool,
}

pub(crate) fn initialize_new_request(
    root: &Path,
    request: &InitializationRequest,
    operation: &DataRootRequest,
) -> Result<InitializationReceipt, String> {
    operation.validate_root(root)?;
    let root = canonical_root(root).map_err(code)?;
    if fs::read_dir(&root)
        .map_err(|_| code(OwnerError::DataRootInvalid))?
        .next()
        .transpose()
        .map_err(|_| code(OwnerError::DataRootInvalid))?
        .is_some()
    {
        // An initialized exact request is resolved, never created a second time.
        return resolve_completed(&root, request, operation);
    }
    let cap = acquire_new(&root, request, operation).map_err(code)?;
    cap.verify().map_err(code)?;
    let store = crate::direct_store::DirectStore::initialize_legacy_layout(&cap)?;
    store.verify_empty()?;
    drop(store);
    cap.verify().map_err(code)?;

    let mut installation_file = create_new_file(&root.join(INSTALLATION_FILE)).map_err(code)?;
    let mut binding = InstallationBinding {
        installation_id: cap.intent.installation,
        installation_incarnation_id: cap.intent.incarnation,
        initialization_id: None,
        native_objects_digest: None,
    };
    let observed = observe_physical_root(&root).map_err(code)?;
    let mut record =
        super::succession::plan_successor(&binding, &observed, cap.intent.executable, None)
            .map_err(code)?;
    record.owner_token = cap.intent.owner_token;
    record.owner_pid = cap.intent.owner_pid;
    record.refresh_digest();
    publish_initial_slot(&root, OWNER_SLOT_A, &record).map_err(code)?;
    publish_initial_slot(&root, OWNER_SLOT_B, &record).map_err(code)?;
    publish_initialized_installation(&root, &mut installation_file, &mut binding, cap.intent.id)
        .map_err(code)?;
    drop(installation_file);
    finish_initialized(cap, record, false)
}

/// Resolve only the original complete layout. Missing files or credentials stay
/// missing; every unknown outcome retains the intent and is never replayed.
pub(crate) fn recover_initialization_request(
    root: &Path,
    request: &InitializationRequest,
    operation: &DataRootRequest,
) -> Result<InitializationReceipt, String> {
    operation.validate_root(root)?;
    let root = canonical_root(root).map_err(code)?;
    if matches!(fs::symlink_metadata(root.join(INTENT)), Err(error) if error.kind() == io::ErrorKind::NotFound)
    {
        return resolve_completed(&root, request, operation);
    }
    let cap = acquire_recovery(&root, request, operation).map_err(code)?;
    cap.verify().map_err(code)?;
    let binding = load_existing_installation(&root).map_err(code)?;
    if binding.initialization_id != Some(cap.intent.id)
        || binding.installation_id != cap.intent.installation
        || binding.installation_incarnation_id != cap.intent.incarnation
    {
        return Err(code(OwnerError::OwnerOperationConflict));
    }
    let (_, record) = newest_valid(&root).map_err(code)?;
    let record = record.ok_or_else(|| code(OwnerError::OwnerRecoveryEvidenceMissing))?;
    // Initialization owns epoch one only. Later root activity cannot be
    // relabelled as initialization recovery, even with the original request id.
    let observed = observe_physical_root(&root).map_err(code)?;
    let expected_generation = match record.lifecycle {
        LifecycleState::Active => 1,
        LifecycleState::Draining => 2,
        LifecycleState::Released => 3,
    };
    if record.epoch != 1
        || record.generation != expected_generation
        || record.previous_epoch != 0
        || record.previous_record_digest != [0; 32]
        || record.owner_token != cap.intent.owner_token
        || record.owner_pid != cap.intent.owner_pid
        || record.installation_id != binding.installation_id
        || record.installation_incarnation_id != binding.installation_incarnation_id
        || record.data_root_id != cap.intent.root
        || record.executable_digest != cap.intent.executable
        || record.canonical_path_digest != observed.canonical_path_digest
        || record.volume_identity_digest != observed.volume_identity_digest
        || (record.lifecycle == LifecycleState::Draining
            && record.drain_reason != super::spec::DrainReasonText::Shutdown)
    {
        return Err(code(OwnerError::OwnerGuardMismatch));
    }
    let recovery = InitializationRecovery { root: &cap };
    let store = crate::direct_store::DirectStore::open_initialization_recovery(&recovery)?;
    store.verify_empty()?;
    drop(store);
    finish_initialized(cap, *record, true)
}

fn finish_initialized(
    cap: InitializingDataRoot,
    record: DurableOwnerRecord,
    replayed: bool,
) -> Result<InitializationReceipt, String> {
    cap.verify().map_err(code)?;
    let native_objects =
        super::installation::retain_native_installation(&cap.root).map_err(code)?;
    let recovery = InitializationRecovery { root: &cap };
    // Credential creation is restricted to the new-initialization child. This
    // final proof uses only the already-existing credential, even after a crash.
    let store = crate::direct_store::DirectStore::open_initialization_recovery(&recovery)?;
    store.verify_empty()?;
    drop(store);
    let mut owner = LiveOwner {
        canonical_root: cap.root.clone(),
        installation_incarnation_id: search_contracts::InstallationIncarnationId::from_bytes(
            record.installation_incarnation_id,
        ),
        data_root_id: search_contracts::DataRootId::from_bytes(record.data_root_id),
        epoch: search_contracts::OwnerEpoch::new(record.epoch)
            .map_err(|_| code(OwnerError::ContractExhausted))?,
        record,
        recovered_previous_active: false,
        poisoned: false,
    };
    if owner.record.lifecycle != LifecycleState::Released {
        owner.begin_drain(DrainReason::Shutdown).map_err(code)?;
        owner.release_cleanly().map_err(code)?;
    }
    cap.verify().map_err(code)?;
    owner.verify_existing().map_err(code)?;
    let receipt = InitializationReceipt {
        namespace: cap.intent.namespace,
        replayed,
    };
    let InitializingDataRoot {
        root,
        intent_file,
        primary,
        sealed,
        root_file,
        ..
    } = cap;
    drop(intent_file);
    fs::remove_file(root.join(INTENT)).map_err(|_| code(OwnerError::OwnerReleaseOutcomeUnknown))?;
    sync_directory(&root);
    if !matches!(fs::symlink_metadata(root.join(INTENT)), Err(error) if error.kind() == io::ErrorKind::NotFound)
    {
        return Err(code(OwnerError::OwnerReleaseOutcomeUnknown));
    }
    drop(owner);
    drop(native_objects);
    drop(root_file);
    drop(sealed);
    drop(primary);
    Ok(receipt)
}

fn resolve_completed(
    root: &Path,
    request: &InitializationRequest,
    operation: &DataRootRequest,
) -> Result<InitializationReceipt, String> {
    crate::development::DataRootGuard::with_inspection_request(
        operation.root_locator(),
        operation,
        |cap| {
            let binding = load_existing_installation(root).map_err(code)?;
            if binding.initialization_id != Some(request.0) {
                return Err(code(OwnerError::OwnerOperationConflict));
            }
            let store = crate::direct_store::DirectStore::open_existing_read_only(cap)?;
            store.verify_catalog()?;
            let namespace = crate::sha256::decode_digest(&store.namespace_id())
                .ok_or_else(|| code(OwnerError::OwnerRecoveryQuarantined))?;
            Ok(InitializationReceipt {
                namespace,
                replayed: true,
            })
        },
    )
}

fn acquire_new(
    root: &Path,
    request: &InitializationRequest,
    operation: &DataRootRequest,
) -> Result<InitializingDataRoot, OwnerError> {
    operation.check()?;
    let root_file = open_bound_directory(root)?;
    if fs::read_dir(root)
        .map_err(|_| OwnerError::DataRootInvalid)?
        .next()
        .transpose()
        .map_err(|_| OwnerError::DataRootInvalid)?
        .is_some()
    {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    let observed = observe_physical_root(root)?;
    let executable = observe_executable()?;
    let (installation, incarnation) = mint_installation_ids(&observed, &executable)?;
    let owner_token = mint_owner_token(&observed, &executable)?;
    let namespace = initialization_namespace(
        request.0,
        installation,
        incarnation,
        *observed.data_root_id.as_bytes(),
        executable,
    )?;
    let intent = InitIntent {
        id: request.0,
        installation,
        incarnation,
        root: *observed.data_root_id.as_bytes(),
        executable,
        namespace,
        owner_token,
        owner_pid: std::process::id(),
    };
    // The exact intent is the first durable effect and arbitrates concurrent init.
    operation.check()?;
    let mut intent_file = create_new_file(&root.join(INTENT))?;
    intent_file
        .write_all(&intent.encode())
        .and_then(|()| intent_file.sync_all())
        .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)?;
    sync_directory(root);
    let primary = create_new_file(&root.join(PRIMARY))?;
    lock_primary(&primary)?;
    let sealed_file = create_new_file(&root.join(SEALED))?;
    sealed_file
        .sync_all()
        .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)?;
    drop(sealed_file);
    let sealed = existing_sealed(root)?;
    // No unrelated state may enter the empty layout after the first proof.
    for entry in fs::read_dir(root).map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)? {
        let name = entry
            .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)?
            .file_name();
        if !matches!(name.to_str(), Some(INTENT | PRIMARY | SEALED)) {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
    }
    assemble_cap(
        root,
        root_file,
        primary,
        sealed,
        intent_file,
        intent,
        operation,
    )
}

fn acquire_recovery(
    root: &Path,
    request: &InitializationRequest,
    operation: &DataRootRequest,
) -> Result<InitializingDataRoot, OwnerError> {
    operation.check()?;
    let root_file = open_bound_directory(root)?;
    let primary = existing_file(&root.join(PRIMARY))?;
    lock_primary(&primary)?;
    let sealed = existing_sealed(root)?;
    let intent_file = existing_file(&root.join(INTENT))?;
    let bytes = read_existing_bytes(&root.join(INTENT), MAX_INTENT)?
        .ok_or(OwnerError::OwnerRecoveryEvidenceMissing)?;
    let intent = InitIntent::decode(&bytes)?;
    if intent.id != request.0 {
        return Err(OwnerError::OwnerOperationConflict);
    }
    // Every other operation barrier is outside this exact initialization.
    crate::catalog_quarantine::check(root).map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
    for name in [
        "control/catalog-quarantine.tmp",
        "control/control.redb",
        "control/source-roots.v1",
        "control/source-roots.tmp",
        "control/source-roots.bak",
    ] {
        if !matches!(fs::symlink_metadata(root.join(name)), Err(error) if error.kind() == io::ErrorKind::NotFound)
        {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
    }
    assemble_cap(
        root,
        root_file,
        primary,
        sealed,
        intent_file,
        intent,
        operation,
    )
}

fn assemble_cap(
    root: &Path,
    root_file: File,
    primary: File,
    sealed: Option<crate::sealed_root_lock::SealedRootLease>,
    intent_file: File,
    intent: InitIntent,
    operation: &DataRootRequest,
) -> Result<InitializingDataRoot, OwnerError> {
    let cap = InitializingDataRoot {
        root: root.to_owned(),
        root_file,
        primary,
        sealed,
        intent_file,
        intent,
        operation: operation.retain(),
    };
    cap.verify()?;
    Ok(cap)
}

fn canonical_root(path: &Path) -> Result<PathBuf, OwnerError> {
    let file = open_bound_directory(path)?;
    let root = fs::canonicalize(path).map_err(|_| OwnerError::DataRootInvalid)?;
    verify_bound_directory(&file, &root)?;
    Ok(root)
}

fn create_new_file(path: &Path) -> Result<File, OwnerError> {
    let mut options = OpenOptions::new();
    options.create_new(true).read(true).write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000).share_mode(0x3);
    }
    options
        .open(path)
        .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)
}

fn existing_file(path: &Path) -> Result<File, OwnerError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| OwnerError::OwnerRecoveryEvidenceMissing)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || super::codec::is_reparse(&metadata)
    {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000).share_mode(0x3);
    }
    let file = options
        .open(path)
        .map_err(|_| OwnerError::OwnerRecoveryEvidenceMissing)?;
    verify_existing_locator(&file, path)?;
    Ok(file)
}

fn lock_primary(file: &File) -> Result<(), OwnerError> {
    file.try_lock().map_err(|error| match error {
        TryLockError::WouldBlock => OwnerError::DataRootAlreadyOwned,
        TryLockError::Error(_) => OwnerError::OwnerRecoveryQuarantined,
    })
}

fn existing_sealed(
    root: &Path,
) -> Result<Option<crate::sealed_root_lock::SealedRootLease>, OwnerError> {
    #[cfg(windows)]
    {
        crate::sealed_root_lock::SealedRootLease::acquire_existing(root)
            .map(Some)
            .map_err(|error| match error {
                crate::sealed_root_lock::SealedRootLockError::AlreadyOwned => {
                    OwnerError::DataRootAlreadyOwned
                }
                _ => OwnerError::OwnerRecoveryQuarantined,
            })
    }
    #[cfg(not(windows))]
    {
        let _ = root;
        Ok(None)
    }
}

fn publish_initial_slot(
    root: &Path,
    name: &str,
    record: &DurableOwnerRecord,
) -> Result<(), OwnerError> {
    let expected = record.encode();
    let path = root.join(name);
    let mut file = create_new_file(&path)?;
    file.write_all(&expected)
        .and_then(|()| file.sync_all())
        .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)?;
    sync_directory(root);
    if read_existing_bytes(&path, super::spec::MAX_STATE_BYTES)? != Some(expected) {
        return Err(OwnerError::OwnerAcquireOutcomeUnknown);
    }
    Ok(())
}

fn initialization_namespace(
    id: [u8; 16],
    installation: [u8; 16],
    incarnation: [u8; 16],
    root: [u8; 16],
    executable: [u8; 32],
) -> Result<[u8; 32], OwnerError> {
    // Closed CBOR array: operation, installation, incarnation, root, executable.
    let parts = [
        id.to_vec(),
        installation.to_vec(),
        incarnation.to_vec(),
        root.to_vec(),
        executable.to_vec(),
    ];
    let values = parts
        .into_iter()
        .map(|bytes| {
            BoundedBytes::new(bytes)
                .map(CanonicalValue::Bytes)
                .map_err(|_| OwnerError::DataRootInvalid)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let value =
        CanonicalValue::Array(BoundedList::new(values).map_err(|_| OwnerError::DataRootInvalid)?);
    let domain = CanonicalDigestDomain::parse("eliot/cbor/data-root-initialization-namespace/v1")
        .map_err(|_| OwnerError::DataRootInvalid)?;
    let limit = DigestInputLimit::new(1024).map_err(|_| OwnerError::DataRootInvalid)?;
    Ok(*blake3_canonical(&domain, &value, limit)
        .map_err(|_| OwnerError::DataRootInvalid)?
        .as_bytes())
}

fn code(error: OwnerError) -> String {
    error.code().to_owned()
}
