//! Minted-once installation binding and exact file readback.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

use search_runtime_owner::OwnerError;

use super::codec::{hex, parse_digest32, parse_id16, push_field, push_line, sync_directory};
use super::observation::{mint_installation_ids, observe_executable, observe_physical_root};
use super::read_existing::read_existing_bytes;
use super::spec::{
    FORMAT_VERSION_LINE, INSTALLATION_FILE, INSTALLATION_MAGIC, MAX_INSTALLATION_BYTES,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct InstallationBinding {
    pub(super) installation_id: [u8; 16],
    pub(super) installation_incarnation_id: [u8; 16],
    pub(super) initialization_id: Option<[u8; 16]>,
    pub(super) native_objects_digest: Option<[u8; 32]>,
}

/// Loads the minted-once installation identity or mints it atomically.
///
/// A corrupt existing file quarantines: regenerating would fork the stable
/// installation identity a copied root must keep proving.
pub(super) fn load_or_create_installation(
    canonical_root: &Path,
) -> Result<InstallationBinding, OwnerError> {
    let path = canonical_root.join(INSTALLATION_FILE);
    match read_existing_bytes(&path, MAX_INSTALLATION_BYTES)? {
        Some(bytes) => parse_installation(&bytes),
        None => mint_installation(canonical_root, &path),
    }
}

/// Reads the existing installation binding without creating a replacement.
pub(super) fn load_existing_installation(root: &Path) -> Result<InstallationBinding, OwnerError> {
    let bytes = read_existing_bytes(&root.join(INSTALLATION_FILE), MAX_INSTALLATION_BYTES)?
        .ok_or(OwnerError::OwnerRecoveryEvidenceMissing)?;
    let binding = parse_installation(&bytes)?;
    if let Some(expected) = binding.native_objects_digest {
        if super::native_bindings::native_objects_digest(root)? != expected {
            return Err(OwnerError::OwnerGuardMismatch);
        }
    }
    Ok(binding)
}

/// Ordinary product opens require the explicit native-bound format; legacy
/// installation evidence is never silently adopted or rewritten.
pub(crate) fn verify_native_installation(root: &Path) -> Result<(), OwnerError> {
    let binding = load_existing_installation(root)?;
    if binding.initialization_id.is_none() || binding.native_objects_digest.is_none() {
        return Err(OwnerError::OwnerRecoveryEvidenceMissing);
    }
    Ok(())
}

/// Pin the admitted layout before children open it, then verify both the
/// persisted profile and the retained identities while replacement is denied.
pub(crate) fn retain_native_installation(
    root: &Path,
) -> Result<super::native_bindings::NativeLayoutPins, OwnerError> {
    let pins = super::native_bindings::NativeLayoutPins::acquire(root)?;
    verify_native_installation(root)?;
    pins.verify(root)?;
    Ok(pins)
}

fn parse_installation(bytes: &[u8]) -> Result<InstallationBinding, OwnerError> {
    if bytes.len() > MAX_INSTALLATION_BYTES {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    let text = core::str::from_utf8(bytes).map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
    if !text.ends_with('\n') {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    let lines: Vec<&str> = text.lines().collect();
    if !matches!(lines.len(), 4 | 7) || lines[0] != INSTALLATION_MAGIC {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    let (initialization_id, native_objects_digest) =
        if lines.len() == 4 && lines[1] == FORMAT_VERSION_LINE {
            (None, None)
        } else if lines.len() == 7
            && lines[1] == "format_version=2"
            && lines[5] == "layout_profile=legacy-direct-root-v2"
        {
            let id = parse_id16(lines[4].strip_prefix("initialization_id="))?;
            if id == [0; 16] {
                return Err(OwnerError::OwnerRecoveryQuarantined);
            }
            (
                Some(id),
                Some(parse_digest32(
                    lines[6].strip_prefix("native_objects_digest="),
                )?),
            )
        } else {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        };
    let installation_id = lines[2]
        .strip_prefix("installation_id=")
        .ok_or(OwnerError::OwnerRecoveryQuarantined)?;
    let incarnation_id = lines[3]
        .strip_prefix("installation_incarnation_id=")
        .ok_or(OwnerError::OwnerRecoveryQuarantined)?;
    Ok(InstallationBinding {
        installation_id: parse_id16(Some(installation_id))?,
        installation_incarnation_id: parse_id16(Some(incarnation_id))?,
        initialization_id,
        native_objects_digest,
    })
}

/// Fill the already-created installation object once, after native identities
/// of every required layout object exist. The initialization intent remains held.
pub(super) fn publish_initialized_installation(
    root: &Path,
    file: &mut File,
    binding: &mut InstallationBinding,
    id: [u8; 16],
) -> Result<(), OwnerError> {
    binding.initialization_id = Some(id);
    binding.native_objects_digest = Some(super::native_bindings::native_objects_digest(root)?);
    let text = format!(
        "{INSTALLATION_MAGIC}\nformat_version=2\ninstallation_id={}\ninstallation_incarnation_id={}\ninitialization_id={}\nlayout_profile=legacy-direct-root-v2\nnative_objects_digest={}\n",
        hex(&binding.installation_id),
        hex(&binding.installation_incarnation_id),
        hex(&id),
        hex(&binding
            .native_objects_digest
            .ok_or(OwnerError::OwnerRecoveryEvidenceMissing)?),
    );
    file.write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)?;
    sync_directory(root);
    if load_existing_installation(root)? != *binding {
        return Err(OwnerError::OwnerAcquireOutcomeUnknown);
    }
    Ok(())
}

fn mint_installation(
    canonical_root: &Path,
    path: &Path,
) -> Result<InstallationBinding, OwnerError> {
    let observed = observe_physical_root(canonical_root)?;
    let executable = observe_executable()?;
    let (installation_id, installation_incarnation_id) =
        mint_installation_ids(&observed, &executable)?;
    let binding = InstallationBinding {
        installation_id,
        installation_incarnation_id,
        initialization_id: None,
        native_objects_digest: None,
    };
    let mut text = String::new();
    push_line(&mut text, INSTALLATION_MAGIC);
    push_line(&mut text, FORMAT_VERSION_LINE);
    push_field(&mut text, "installation_id", &hex(&binding.installation_id));
    push_field(
        &mut text,
        "installation_incarnation_id",
        &hex(&binding.installation_incarnation_id),
    );
    // First writer wins; an already-present file is re-read, never replaced.
    match OpenOptions::new().create_new(true).write(true).open(path) {
        Ok(mut file) => {
            file.write_all(text.as_bytes())
                .map_err(|_| OwnerError::DataRootInvalid)?;
            file.sync_all().map_err(|_| OwnerError::DataRootInvalid)?;
            drop(file);
            sync_directory(canonical_root);
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(OwnerError::DataRootInvalid),
    }
    let bytes = read_existing_bytes(path, MAX_INSTALLATION_BYTES)?
        .ok_or(OwnerError::OwnerAcquireOutcomeUnknown)?;
    parse_installation(&bytes)
}
