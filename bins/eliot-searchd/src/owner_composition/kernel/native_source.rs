//! Root-purpose acquisition: native facts and original context precede Source admission.
use super::{FreshRootContext, FreshRootError, Pin, RequalifiedFreshRoot, SourceRegistry,
    Work, V, admission, array, bytes, c, hash, invalid, reference};
use eliot_searchd::native_file::{self as nf, NormalizedNameKind, Observation, ObservationError, SyncReadFile};
use search_source_admission::{UnvalidatedObservationInput, evaluation_v2::{DEFAULT_EVALUATION_LIMITS, evaluate_pre_read_v2},
    observation_v2::MAX_OBSERVATION_PREIMAGE_BYTES, policy_v2::CheckpointFailureV2};
use search_source_identity::{FilesystemIdentityProfile, IdentityObservation, ObservationConfidence,
    PathObservation, SourceObservation, StableIdentityEvidence, StableIdentityKey,
    derive_canonical_path_key, validate_identity_observation};
use search_source_provenance::OriginalAdmissionV2;
use std::{cell::RefCell, fs::File, io::{Read, Seek, SeekFrom}, path::{Path, PathBuf}, sync::Arc};

const fn refused(code: &'static str) -> FreshRootError { FreshRootError::Source(code) }

pub struct AcquiredNativeSource<'view, 'operation, 'owner> {
    qualified: RequalifiedFreshRoot<'view, 'operation, 'owner>,
    pins: Vec<Pin>, leaf: SyncReadFile, path: PathBuf, observed: Observation,
    location: nf::NativeLocationClass, name: String,
    identity: search_source_identity::ValidatedIdentityObservation,
    binding: SourceObservation, original: OriginalAdmissionV2,
}
impl AcquiredNativeSource<'_, '_, '_> {
    pub(crate) const fn root(&self) -> &search_source_registry::root::RootRecord { self.qualified.root() }
    pub(crate) const fn identity_observation(&self) -> &search_source_identity::ValidatedIdentityObservation { &self.identity }
    pub(crate) const fn binding_observation(&self) -> &SourceObservation { &self.binding }
    pub(crate) const fn original(&self) -> &OriginalAdmissionV2 { &self.original }
    pub(crate) fn check_current(&self, registry: &SourceRegistry) -> Result<(), FreshRootError> {
        check_root_state(&self.qualified, registry)?;
        check_leaf(self.qualified.native, &self.pins, &self.leaf, &self.path,
            self.observed, self.location, &self.name)
    }
}

impl<'operation, 'owner> FreshRootContext<'operation, 'owner> {
    pub(crate) fn source_checkpoint(&self) -> Result<(), FreshRootError> { self.checkpoint() }
    pub(crate) fn source_full_checkpoint(&self) -> Result<(), FreshRootError> { self.verify_owner() }
    pub(crate) fn source_owner_scope(&self) -> Result<search_source_registry::api::SourceOwnerScopeV2, FreshRootError> {
        self.checkpoint()?;
        let incarnation = self.operation.borrow_mut().with_binding(|binding, _, _, _|
            Ok(binding.installation.owner_binding().installation_incarnation_id)).map_err(FreshRootError::Native)?;
        Ok(search_source_registry::api::SourceOwnerScopeV2 {
            system_id: c::OpaqueId::new(env!("CARGO_PKG_NAME")).map_err(|_| invalid())?,
            installation_incarnation: c::InstallationIncarnationId::from_bytes(incarnation), epoch: self.epoch,
        })
    }
    fn source_io<T>(&self, operation: impl FnOnce(&dyn Fn(bool) -> Result<(), ObservationError>) -> Result<T, ObservationError>)
        -> Result<T, FreshRootError> {
        self.checkpoint()?;
        let first = RefCell::new(None);
        let check = |before| {
            let mut op = self.operation.borrow_mut();
            let result = if before { op.migration_held_observer_dispatch() } else { op.migration_checkpoint() };
            result.map_err(|error| { if first.borrow().is_none() { *first.borrow_mut() = Some(error); }
                ObservationError::ObservationFailed })
        };
        let value = operation(&check);
        if let Some(error) = first.into_inner() { return Err(FreshRootError::Native(error)); }
        self.checkpoint()?;
        value.map_err(FreshRootError::Observation)
    }
    fn source_directory(&self, path: &Path) -> Result<Pin, FreshRootError> {
        self.charge(Work::Open)?;
        let (directory, observed) = self.source_io(|check| {
            check(true)?;
            #[cfg(windows)]
            let file = { use std::os::windows::fs::OpenOptionsExt;
                std::fs::OpenOptions::new().access_mode(0x0012_00a9).share_mode(1)
                    .custom_flags(0x0220_0000).open(path) };
            #[cfg(not(windows))]
            let file: std::io::Result<File> = Err(std::io::ErrorKind::Unsupported.into());
            check(false)?;
            nf::QualifiedDirectory::qualify_with_check(Arc::new(file.map_err(|_| ObservationError::ObservationFailed)?), check)?
                .ok_or(ObservationError::UnsupportedHandle)
        })?;
        if self.source_io(|check| nf::case_sensitive_directory_with_check(directory.file(), check))? {
            return Err(refused("NATIVE_SOURCE_CASE_PROFILE_UNSUPPORTED"));
        }
        Ok(Pin { path: path.to_owned(), directory, observed })
    }
    fn source_name(&self, file: &File, kind: NormalizedNameKind) -> Result<String, FreshRootError> {
        self.charge(Work::Allocate(nf::NORMALIZED_NAME_MAX_UTF16 * 4))?;
        self.source_io(|check| nf::normalized_name_with_check(file, kind, check))
    }
    fn source_data_root_name(&self) -> Result<String, FreshRootError> {
        self.checkpoint()?;
        self.operation.borrow_mut().with_binding(|binding, request, work, _| {
            work.read(request, nf::NORMALIZED_NAME_MAX_UTF16 * 4)?;
            let work = RefCell::new(work);
            let first = RefCell::new(None);
            let check = |before| {
                let result = if before { work.borrow_mut().call(request, || Ok(())) } else { request.check() };
                result.map_err(|error| { if first.borrow().is_none() { *first.borrow_mut() = Some(error); }
                    ObservationError::ObservationFailed })
            };
            let name = nf::normalized_name_with_check(binding.bound_root, NormalizedNameKind::VolumeGuid, &check);
            if let Some(error) = first.into_inner() { return Err(error); }
            name.map_err(|_| search_runtime_owner::OwnerError::OwnerGuardMismatch)
        }).map_err(FreshRootError::Native)
    }
    #[expect(clippy::too_many_lines, reason = "Single Root-to-Source acquisition keeps its retained FD and original context across pre/read/post phases")]
    pub(in super::super::super::super) fn acquire_source_v2<'view>(&'view self, root_path: &Path, id: c::RootBindingId,
        registry: &SourceRegistry, policy: &super::super::super::super::direct_policy_v2::CapturedPolicyV2,
        relative: &[u8], profile: FilesystemIdentityProfile,
    ) -> Result<AcquiredNativeSource<'view, 'operation, 'owner>, FreshRootError> {
        let (policy, policy_reference) = (&policy.retained, &policy.reference);
        let qualified = self.requalify_existing_v2(root_path, id, registry, policy, policy_reference)?;
        let relative = std::str::from_utf8(relative).map_err(|_| invalid())?;
        let mut path = root_path.to_owned();
        self.charge(Work::AllocateLocator(relative.len().saturating_mul(128).saturating_add(8192)))?;
        let mut pins = vec![self.source_directory(&path)?];
        let root_observed = pins[0].observed;
        if search_source_identity::ntfs_directory::ntfs_directory_digest(root_observed.volume_serial,
            root_observed.file_index, root_observed.creation_time).map_err(|_| invalid())?
            != qualified.root().canonical_root_digest { return Err(invalid()); }
        let parts = relative.split('/').collect::<Vec<_>>();
        self.charge(Work::Allocate(nf::CHILD_MEMBERSHIP_BUFFER_BYTES))?;
        let mut scratch = nf::ChildMembershipScratch::new();
        for part in &parts[..parts.len().saturating_sub(1)] {
            path.push(part);
            let next = self.source_directory(&path)?;
            directory_member(self, pins.last().ok_or_else(invalid)?.directory.file(), next.directory.file(), &mut scratch)?;
            pins.push(next);
        }
        path.push(parts.last().ok_or_else(invalid)?);
        self.charge(Work::Open)?;
        let leaf = self.source_io(|check| SyncReadFile::open_no_follow(&path, check))?;
        let observed = self.source_io(|check| nf::observe_with_links_with_check(leaf.file(), check))?;
        if observed.1 != 1 { return Err(refused("NATIVE_SOURCE_HARDLINK_DENIED")); }
        let observed = observed.0;
        file_member(self, pins.last().ok_or_else(invalid)?.directory.file(), leaf.file(), &mut scratch)?;
        let location = self.source_io(|check| nf::location_with_check(&leaf, check))?;
        let location_text = match location {
            nf::NativeLocationClass::LocalFixed => "local-fixed",
            nf::NativeLocationClass::LocalRemovable => "local-removable",
            nf::NativeLocationClass::NetworkShare => "remote",
            nf::NativeLocationClass::Remote => "remote",
            _ => return Err(refused("NATIVE_SOURCE_LOCATION_UNAVAILABLE")),
        };
        let name = self.source_name(leaf.file(), NormalizedNameKind::VolumeGuid)?;
        let owned_prefix = format!("{}\\", self.source_data_root_name()?.trim_end_matches('\\'));
        if name.starts_with(&owned_prefix) { return Err(refused("NATIVE_SOURCE_OWN_STATE_DENIED")); }
        let root_name = self.source_name(pins[0].directory.file(), NormalizedNameKind::VolumeGuid)?;
        let root_prefix = format!("{}\\", root_name.trim_end_matches('\\'));
        let canonical = name.strip_prefix(&root_prefix).ok_or_else(invalid)?.replace('\\', "/");
        let dos = self.source_name(leaf.file(), NormalizedNameKind::Dos)?;
        let class = crate::source_classification::classify(Path::new(&dos)).map_err(|_| invalid())?;
        let mut raw = metadata_input(class, location_text, observed.length, true);
        let mut probe = || self.checkpoint().map_err(|e| if matches!(e,
            FreshRootError::Native(search_runtime_owner::OwnerError::OwnerCancelledBeforeMutation)) {
                CheckpointFailureV2::Cancelled } else { CheckpointFailureV2::DeadlineExceeded });
        let pre = admission::original_inputs_v2::reconstitute_observation_v2(&raw, Some(canonical.as_bytes()),
            policy.compiled(), MAX_OBSERVATION_PREIMAGE_BYTES, &mut probe).map_err(FreshRootError::Admission)?;
        let verdict = evaluate_pre_read_v2(policy.compiled(), &pre, DEFAULT_EVALUATION_LIMITS, &mut probe)
            .map_err(FreshRootError::Admission)?;
        if !verdict.read_allowed() { return Err(refused("NATIVE_SOURCE_PRE_READ_REFUSED")); }
        let guid = volume_guid(&name)?;
        let volume = search_source_identity::ntfs_volume_identity_v1(&guid, observed.volume_serial).map_err(|_| invalid())?;
        let physical = search_source_identity::ntfs_file_identity_v1(observed.file_index, observed.creation_time).map_err(|_| invalid())?;
        let purpose = hash("eliot/cbor/source-registry/root-native-purpose/v2", &array(vec![
            self.owner.clone(), self.request.clone(), bytes(id.as_bytes())?, bytes(qualified.root().canonical_root_digest.as_bytes())?,
            V::U64(qualified.root().record_revision.get()), V::U64(self.epoch.get()),
            bytes(policy.fingerprint().as_bytes())?, bytes(pre.fingerprint().as_bytes())?])?)?;
        let mut backend = SourceBackend { native: self, qualified: &qualified, registry, pins: &pins,
            leaf: &leaf, path: &path, observed, location, name: &name, relative: &canonical,
            physical, purpose };
        let length = usize::try_from(observed.length).map_err(|_| invalid())?;
        let request = search_safe_reader::AcquisitionRequestV2 {
            relative_path: search_safe_reader::RelativePathToken::new(&canonical, search_safe_reader::DEFAULT_SAFE_READ_LIMITS).map_err(|_| invalid())?,
            expected_root_identity_digest: qualified.root().canonical_root_digest,
            expected_stable_file_identity_digest: physical, expected_root_purpose_digest: purpose,
            range: search_safe_reader::ReadRange { offset: 0, length, require_eof: true },
        };
        let result = search_safe_reader::acquire_with_cancel_v2(&mut backend, &request,
            search_safe_reader::DEFAULT_SAFE_READ_LIMITS, &mut || self.checkpoint().is_err());
        if result.is_err() { self.checkpoint()?; }
        let result = result.map_err(|_| refused("NATIVE_SOURCE_STABLE_READ_REFUSED"))?;
        let body = result.into_bytes();
        self.charge(Work::Compare(body.len()))?;
        let content = c::blake3_exact_bytes(&body, c::Blake3ExactInputLimit::new(body.len()).map_err(|_| invalid())?).map_err(|_| invalid())?;
        raw = metadata_input(crate::source_classification::narrow_with_bytes(class, &body), location_text, observed.length, false);
        let post = admission::original_inputs_v2::reconstitute_observation_v2(&raw, Some(canonical.as_bytes()),
            policy.compiled(), MAX_OBSERVATION_PREIMAGE_BYTES, &mut probe).map_err(FreshRootError::Admission)?;
        let original = OriginalAdmissionV2::retain(Arc::clone(policy), &post, &mut probe).map_err(FreshRootError::Admission)?;
        if original.receipt().decision().outcome() != admission::AdmissionOutcome::Allow {
            return Err(refused("NATIVE_SOURCE_POST_READ_REFUSED"));
        }
        let evidence = hash("eliot/cbor/native-source-observation/v1", &array(vec![bytes(purpose.as_bytes())?,
            bytes(volume.as_bytes())?, bytes(physical.as_bytes())?, bytes(content.as_bytes())?,
            bytes(post.fingerprint().as_bytes())?])?)?;
        let path_key = derive_canonical_path_key(&PathObservation { root_binding_id: id,
            root_relative_lookup_path: canonical.clone(), profile_revision: profile.revision(),
            profile_schema_digest: profile.schema_digest(), normalization_attested: true }, profile).map_err(|_| invalid())?;
        let identity = validate_identity_observation(IdentityObservation { path_key,
            stable_evidence: StableIdentityEvidence::Exact(StableIdentityKey::Filesystem {
                volume_identity: volume, file_identity: physical, generation: None }),
            content_digest_hint: Some(content), metadata_generation: None, byte_length_hint: Some(observed.length),
            claimed_source_id: None, candidate_catalog_revision: c::CatalogRevision::new(registry.revision()),
            profile_revision: profile.revision(), profile_schema_digest: profile.schema_digest(),
            confidence: ObservationConfidence::Exact, evidence_digest: evidence }, profile).map_err(|_| invalid())?;
        let binding = SourceObservation { root_binding_id: id,
            relative_path: search_source_identity::CanonicalRelativePath::new(canonical, search_source_identity::DEFAULT_IDENTITY_LIMITS).map_err(|_| invalid())?,
            stable_file_identity_digest: Some(physical), content_digest: content, content_bytes: observed.length,
            observation_receipt: reference(evidence)? };
        let acquired = AcquiredNativeSource { qualified, pins, leaf, path, observed, location, name, identity, binding, original };
        acquired.check_current(registry)?;
        Ok(acquired)
    }
}

fn volume_guid(name: &str) -> Result<String, FreshRootError> {
    if name.len() < 49 || !name.is_char_boundary(49) || !name.starts_with(r"\\?\Volume{") { return Err(invalid()); }
    Ok(format!("{}{}{}", &name[..11], name[11..47].to_ascii_lowercase(), &name[47..49]))
}

fn metadata_input(class: crate::source_classification::ClassifiedSource, location: &str, size: u64, pre: bool) -> UnvalidatedObservationInput {
    UnvalidatedObservationInput { locator_class: "normalized-file".into(), location_class: location.into(), source_kind: "file".into(),
        source_class: class.source_class.as_str().into(), byte_size: Some(size), is_generated: Some(class.is_generated),
        is_vendor: Some(class.is_vendor), is_binary: (!pre).then_some(class.is_binary), is_system: Some(class.is_system),
        sensitivity: Some(class.sensitivity.as_str().into()), detector_id: Some("detector:native-path-bytes-v1".into()),
        profile_id: Some(admission::BASELINE_PROFILE.into()),
        unavailable_fields: if pre { vec!["binary-flag".into()] } else { vec![] }, unknown_fields: vec![] }
}

fn directory_member(native: &FreshRootContext<'_, '_>, parent: &File, child: &File,
    scratch: &mut nf::ChildMembershipScratch) -> Result<(), FreshRootError> {
    membership(native, parent, child, scratch, true)
}
fn file_member(native: &FreshRootContext<'_, '_>, parent: &File, child: &File,
    scratch: &mut nf::ChildMembershipScratch) -> Result<(), FreshRootError> {
    membership(native, parent, child, scratch, false)
}

fn membership(native: &FreshRootContext<'_, '_>, parent: &File, child: &File,
    scratch: &mut nf::ChildMembershipScratch, directory: bool) -> Result<(), FreshRootError> {
    let first = RefCell::new(None);
    let process = |count| native.charge(Work::Compare(count)).map_err(|error| {
        if first.borrow().is_none() { *first.borrow_mut() = Some(error); }
        ObservationError::ObservationFailed
    });
    let result = native.source_io(|check| if directory {
        nf::verify_direct_directory_child_with_check(parent, child, scratch, nf::CHILD_MEMBERSHIP_MAX_ENTRIES, check, &process)
    } else {
        nf::verify_direct_file_child_with_check(parent, child, scratch, nf::CHILD_MEMBERSHIP_MAX_ENTRIES, check, &process)
    });
    if let Some(error) = first.into_inner() { return Err(error); }
    result.map(|_| ())
}

fn check_leaf(native: &FreshRootContext<'_, '_>, pins: &[Pin], leaf: &SyncReadFile, path: &Path,
    observed: Observation, location: nf::NativeLocationClass, name: &str) -> Result<(), FreshRootError> {
    for pin in pins {
        let current = native.source_directory(&pin.path)?;
        if (current.observed.volume_serial, current.observed.file_index, current.observed.creation_time)
            != (pin.observed.volume_serial, pin.observed.file_index, pin.observed.creation_time) { return Err(invalid()); }
        if native.source_io(|check| nf::case_sensitive_directory_with_check(pin.directory.file(), check))? { return Err(invalid()); }
    }
    native.charge(Work::Open)?;
    let current = native.source_io(|check| SyncReadFile::open_no_follow(path, check))?;
    for file in [current.file(), leaf.file()] {
        let after = native.source_io(|check| nf::observe_with_links_with_check(file, check))?;
        if after != (observed, 1) { return Err(invalid()); }
    }
    if native.source_io(|check| nf::location_with_check(leaf, check))? != location
        || native.source_name(leaf.file(), NormalizedNameKind::VolumeGuid)? != name { return Err(invalid()); }
    native.checkpoint()
}

// Registry/policy cannot mutate through the read backend's immutable borrow;
// Native custody and both exclusions remain owned by the original operation.
// The source pins below independently re-open current READ/traverse locators.
fn check_root_state(qualified: &RequalifiedFreshRoot<'_, '_, '_>, registry: &SourceRegistry) -> Result<(), FreshRootError> {
    qualified.native.checkpoint()?;
    if registry.roots().get(&qualified.root.root_binding_id) != Some(&qualified.root)
        || qualified.policy.fingerprint() != qualified.root.policy_fingerprint { return Err(invalid()); }
    Ok(())
}

struct SourceBackend<'a, 'view, 'operation, 'owner> {
    native: &'view FreshRootContext<'operation, 'owner>, qualified: &'a RequalifiedFreshRoot<'view, 'operation, 'owner>,
    registry: &'a SourceRegistry, pins: &'a [Pin], leaf: &'a SyncReadFile, path: &'a Path,
    observed: Observation, location: nf::NativeLocationClass, name: &'a str, relative: &'a str,
    physical: c::Blake3Digest32, purpose: c::Blake3Digest32,
}
impl search_safe_reader::AcquisitionBackendV2 for SourceBackend<'_, '_, '_, '_> {
    type Handle = ();
    type BackendError = String;
    fn open_final(&mut self, request: &search_safe_reader::FinalHandleOpenRequest) -> Result<(), String> {
        if request.expected_root_identity_digest != self.qualified.root().canonical_root_digest
            || request.relative_path.as_str() != self.relative { return Err("NATIVE_SOURCE_BINDING_MISMATCH".into()); }
        check_root_state(self.qualified, self.registry).map_err(|e| e.to_string())
    }
    fn inspect(&mut self, &(): &()) -> Result<search_safe_reader::AcquisitionMetadataV2, String> {
        check_root_state(self.qualified, self.registry).map_err(|e| e.to_string())?;
        check_leaf(self.native, self.pins, self.leaf, self.path, self.observed, self.location, self.name).map_err(|e| e.to_string())?;
        Ok(search_safe_reader::AcquisitionMetadataV2 { root_identity_digest: self.qualified.root().canonical_root_digest,
            stable_file_identity_digest: self.physical, kind: search_safe_reader::FinalHandleKind::RegularFile,
            source_bytes: self.observed.length, final_object_is_reparse: false, ancestor_reparse_observed: false,
            root_purpose_digest: self.purpose, change_token: c::OpaqueId::new(format!("native:{}:{}", self.observed.file_index, self.observed.last_write_time)).map_err(|_| "NATIVE_SOURCE_TOKEN")?,
            metadata_receipt: Some(c::ReceiptRef::new(format!("native-source-metadata:{}", self.purpose)).map_err(|_| "NATIVE_SOURCE_RECEIPT")?) })
    }
    fn read_exact_at(&mut self, &(): &(), offset: u64, length: usize) -> Result<search_safe_reader::AdapterRead, String> {
        self.native.charge(Work::Allocate(length)).map_err(|e| e.to_string())?;
        let mut buffer = Vec::new(); buffer.try_reserve_exact(length).map_err(|_| "NATIVE_SOURCE_MEMORY")?; buffer.resize(length, 0);
        let mut file = self.leaf.file();
        self.native.source_io(|check| { check(true)?; let result = file.seek(SeekFrom::Start(offset)); check(false)?;
            result.map(|_| ()).map_err(|_| ObservationError::ObservationFailed) }).map_err(|e| e.to_string())?;
        let mut done = 0;
        while done < length {
            self.native.charge(Work::Read(length - done)).map_err(|e| e.to_string())?;
            let read = self.native.source_io(|check| { check(true)?; let result = file.read(&mut buffer[done..]); check(false)?;
                result.map_err(|_| ObservationError::ObservationFailed) }).map_err(|e| e.to_string())?;
            if read == 0 { return Err("NATIVE_SOURCE_SHORT_READ".into()); } done += read;
        }
        self.native.charge(Work::Read(1)).map_err(|e| e.to_string())?;
        let mut extra = [0];
        let eof = self.native.source_io(|check| { check(true)?; let result = file.read(&mut extra); check(false)?;
            result.map_err(|_| ObservationError::ObservationFailed) }).map_err(|e| e.to_string())?;
        if eof != 0 { return Err("NATIVE_SOURCE_EOF_MISMATCH".into()); }
        Ok(search_safe_reader::AdapterRead { bytes: buffer,
            read_receipt: Some(c::ReceiptRef::new(format!("native-source-read:{}:{length}", self.purpose)).map_err(|_| "NATIVE_SOURCE_RECEIPT")?) })
    }
    fn map_backend_error(_: &String) -> search_safe_reader::SafeReadError { search_safe_reader::SafeReadError::BackendFailure }
}
