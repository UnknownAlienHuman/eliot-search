//! Qdrant directory and child wiring for the same native process Root custody.
use super::super::super::super::storage_acl::OriginalContext;
use super::NativeProcessRootCustody;
use eliot_searchd::native_file::{
    self, ChildMembershipScratch, QualifiedDirectory, StorageAclError, StorageSecurityContext,
    StorageSecurityWork,
};
use search_qdrant_supervisor::{ContainedLaunchError, LaunchPlan, SecretMaterial, SpawnedChild};
use search_runtime_owner::OwnerError;
use std::{
    cell::RefCell,
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    sync::Arc,
};
const QDRANT_DATA_DIRECTORY_NAME: &str = "qdrant-data";
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
const FILE_SHARE_READ: u32 = 1;
/// A directory qualified as a child of this exact retained Root.
pub(in super::super) struct NativeQdrantDataDirectory {
    directory: QualifiedDirectory,
    root_identity: search_control_redb::JournalIdentity,
    identity_digest: search_contracts::Blake3Digest32,
}

impl NativeQdrantDataDirectory {
    pub(crate) const fn identity_digest(&self) -> search_contracts::Blake3Digest32 {
        self.identity_digest
    }
}

impl NativeProcessRootCustody {
    /// Measures the requested artifact on a held, non-writable, non-deletable
    /// file. The pin stays with the runtime through child shutdown. Native
    /// launch independently verifies the hash and version before spawn.
    pub(crate) fn measure_qdrant_artifact(
        &mut self,
        path: &Path,
        expected_sha256: search_contracts::Sha256Digest32,
        expected_version: &str,
    ) -> Result<(search_qdrant_supervisor::QualifiedArtifact, File), OwnerError> {
        use super::super::super::super::native_objects;
        use search_contracts::{Blake3Digest32, Sha256Digest32};
        let root = &mut self.owner.owner;
        root.verify()?;
        if !path.is_absolute()
            || expected_version != search_qdrant_supervisor::QUALIFIED_QDRANT_VERSION
        {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        let mut file =
            native_objects::open_initialization(&root.request, &mut root.work, path, false)?;
        let before = root.work.call(&root.request, || {
            native_file::observe(&file).map_err(|_| OwnerError::OwnerGuardMismatch)
        })?;
        if before.length != search_qdrant_supervisor::QUALIFIED_EXE_BYTES {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        let body = native_objects::read_bytes(
            &root.request,
            &mut root.work,
            &mut file,
            usize::try_from(search_qdrant_supervisor::QUALIFIED_EXE_BYTES)
                .map_err(|_| OwnerError::ContractExhausted)?,
        )?;
        root.work.read(&root.request, body.len())?;
        let measured = root.work.call(&root.request, || {
            Ok(Sha256Digest32::from_bytes(
                search_qdrant_supervisor::sha256_bytes(&body),
            ))
        })?;
        drop(body);
        let after = root.work.call(&root.request, || {
            native_file::observe(&file).map_err(|_| OwnerError::OwnerGuardMismatch)
        })?;
        if before != after || measured != expected_sha256 {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        // This transcript identifies the held native object, not the locator.
        let mut identity = blake3::Hasher::new();
        identity.update(b"eliot-search/native-qdrant-file/v1\0");
        identity.update(&before.volume_serial.to_be_bytes());
        identity.update(&before.file_index.to_be_bytes());
        identity.update(&before.creation_time.to_be_bytes());
        let artifact = search_qdrant_supervisor::admit_pinned_windows_artifact(
            Blake3Digest32::from_bytes(*identity.finalize().as_bytes()),
            measured,
            before.length,
        )
        .map_err(|_| OwnerError::OwnerGuardMismatch)?;
        root.verify()?;
        Ok((artifact, file))
    }

    /// Creates or verifies Qdrant's owner-private data directory under this
    /// exact retained Root, then returns a qualified same-handle directory pin.
    /// The supervisor's contained-spawn owner must consume this exact handle.
    pub(in super::super) fn prepare_qdrant_data_directory(
        &mut self,
    ) -> Result<(NativeQdrantDataDirectory, PathBuf, [u8; 32]), OwnerError> {
        let root_identity = self.journal_identity();
        let direct = &mut self.owner;
        let root = &mut direct.owner;
        root.verify()?;
        direct
            .control
            .verify(&root.request, &mut root.work, &root.root)?;
        let storage_acl = root
            .storage_acl
            .as_ref()
            .ok_or(OwnerError::OwnerStorageAclUnqualified)?;
        let user_scope = storage_acl.current_user_scope(&root.request, &mut root.work)?;
        let root_owner = storage_acl.root_owner_observation()?.clone();
        let path = root.root.join(QDRANT_DATA_DIRECTORY_NAME);

        {
            root.effect_observed = true;
            let mut context = OriginalContext::new(&root.request, &mut root.work);
            match native_file::create_owner_private_directory(&path, &root_owner, &mut context) {
                Ok(()) | Err(StorageAclError::AlreadyExists) => {}
                Err(StorageAclError::Context(error)) => return Err(error),
                Err(StorageAclError::CreateOutcomeUnknown(_)) => {
                    return Err(OwnerError::OwnerAcquireOutcomeUnknown);
                }
                Err(_) => return Err(OwnerError::OwnerStorageAclUnqualified),
            }
            context.checkpoint()?;
        }

        let meter = RefCell::new(OriginalContext::new(&root.request, &mut root.work));
        meter.borrow_mut().charge(StorageSecurityWork::Open)?;
        let file = open_qdrant_directory(&path)?;
        meter.borrow_mut().checkpoint()?;
        let retained = Arc::new(file);
        let check = |dispatch| {
            let mut context = meter.borrow_mut();
            let result = if dispatch {
                context.charge(StorageSecurityWork::NativeCall)
            } else {
                context.checkpoint()
            };
            result.map_err(|_| native_file::ObservationError::ObservationFailed)
        };
        let process = |bytes| {
            meter
                .borrow_mut()
                .charge(StorageSecurityWork::Process(bytes))
                .map_err(|_| native_file::ObservationError::ObservationFailed)
        };
        let qualified =
            native_file::QualifiedDirectory::qualify_with_check(Arc::clone(&retained), &check);
        let Some((directory, identity)) = meter
            .borrow_mut()
            .finish(qualified, OwnerError::OwnerStorageAclUnqualified)?
        else {
            return Err(OwnerError::OwnerStorageAclUnqualified);
        };
        let mut scratch = ChildMembershipScratch::new();
        let member = native_file::verify_direct_directory_child_with_check(
            &root.bound_root,
            directory.file(),
            &mut scratch,
            native_file::CHILD_MEMBERSHIP_MAX_ENTRIES,
            &check,
            &process,
        );
        meter
            .borrow_mut()
            .finish(member, OwnerError::OwnerStorageAclUnqualified)?;
        let acl = native_file::observe_owner_private_acl(
            directory.file(),
            user_scope,
            &mut *meter.borrow_mut(),
        )
        .map_err(|_| OwnerError::OwnerStorageAclUnqualified)?;
        if acl.semantic_fingerprint() != root_owner.semantic_fingerprint() {
            return Err(OwnerError::OwnerStorageAclUnqualified);
        }
        meter
            .borrow_mut()
            .charge(StorageSecurityWork::Process(64))?;
        let mut identity_hash = blake3::Hasher::new();
        identity_hash.update(b"eliot-search/native-qdrant-directory/v1\0");
        identity_hash.update(&identity.volume_serial.to_be_bytes());
        identity_hash.update(&identity.file_index.to_be_bytes());
        identity_hash.update(&identity.creation_time.to_be_bytes());
        let identity_digest =
            search_contracts::Blake3Digest32::from_bytes(*identity_hash.finalize().as_bytes());
        let _ = meter.into_inner();
        root.verify()?;
        direct
            .control
            .verify(&root.request, &mut root.work, &root.root)?;
        Ok((
            NativeQdrantDataDirectory {
                directory,
                root_identity,
                identity_digest,
            },
            path,
            user_scope,
        ))
    }

    /// Consumes the actual owner-private directory pin through the existing
    /// native Qdrant Job producer. No PID lookup or alternate process path is
    /// available from this custody.
    pub(in super::super) fn spawn_qdrant_child(
        &mut self,
        plan: &LaunchPlan,
        secret: &SecretMaterial,
        data_directory: &NativeQdrantDataDirectory,
        current_user_digest: [u8; 32],
        live_secret_check: &mut impl FnMut(
            &search_qdrant_supervisor::SecretLeaseEvidence,
        ) -> Result<(), OwnerError>,
    ) -> Result<
        SpawnedChild,
        ContainedLaunchError<eliot_searchd::native_qdrant_job::NativeQdrantJobError<OwnerError>>,
    > {
        if data_directory.root_identity != self.journal_identity() {
            return Err(ContainedLaunchError::Port(
                eliot_searchd::native_qdrant_job::NativeQdrantJobError::DataDirectoryMismatch,
            ));
        }
        let direct = &mut self.owner;
        let root = &mut direct.owner;
        root.verify().map_err(|error| {
            ContainedLaunchError::Port(
                eliot_searchd::native_qdrant_job::NativeQdrantJobError::Context(error),
            )
        })?;
        direct
            .control
            .verify(&root.request, &mut root.work, &root.root)
            .map_err(|error| {
                ContainedLaunchError::Port(
                    eliot_searchd::native_qdrant_job::NativeQdrantJobError::Context(error),
                )
            })?;
        let mut context = OriginalContext::new(&root.request, &mut root.work);
        eliot_searchd::native_qdrant_job::spawn_qualified(
            plan,
            secret,
            data_directory.directory.file(),
            current_user_digest,
            &mut context,
            live_secret_check,
        )
    }
}
fn open_qdrant_directory(path: &Path) -> Result<File, OwnerError> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;

        let mut options = OpenOptions::new();
        options
            .read(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
            .share_mode(FILE_SHARE_READ);
        options
            .open(path)
            .map_err(|_| OwnerError::OwnerGuardMismatch)
    }
    #[cfg(not(windows))]
    {
        let _ = (
            path,
            FILE_FLAG_OPEN_REPARSE_POINT,
            FILE_FLAG_BACKUP_SEMANTICS,
            FILE_SHARE_READ,
        );
        Err(OwnerError::OwnerStorageAclUnqualified)
    }
}
