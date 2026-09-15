//! Minted-once installation binding and exact file readback.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use search_runtime_owner::OwnerError;

use super::codec::{hex, parse_id16, push_field, push_line, sync_directory};
use super::observation::{
    mint_installation_ids, observe_executable, observe_physical_root,
};
use super::spec::{
    FORMAT_VERSION_LINE, INSTALLATION_FILE, INSTALLATION_MAGIC,
    MAX_INSTALLATION_BYTES,
};

pub(super) struct InstallationBinding {
    pub(super) installation_id: [u8; 16],
    pub(super) installation_incarnation_id: [u8; 16],
}

/// Loads the minted-once installation identity or mints it atomically.
///
/// A corrupt existing file quarantines: regenerating would fork the stable
/// installation identity a copied root must keep proving.
pub(super) fn load_or_create_installation(
    canonical_root: &Path,
) -> Result<InstallationBinding, OwnerError> {
    let path = canonical_root.join(INSTALLATION_FILE);
    match fs::read(&path) {
        Ok(bytes) => parse_installation(&bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            mint_installation(canonical_root, &path)
        }
        Err(_) => Err(OwnerError::DataRootInvalid),
    }
}

fn parse_installation(bytes: &[u8]) -> Result<InstallationBinding, OwnerError> {
    if bytes.len() > MAX_INSTALLATION_BYTES {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    let text = core::str::from_utf8(bytes)
        .map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
    if !text.ends_with('\n') {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() != 4
        || lines[0] != INSTALLATION_MAGIC
        || lines[1] != FORMAT_VERSION_LINE
    {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    let installation_id = lines[2]
        .strip_prefix("installation_id=")
        .ok_or(OwnerError::OwnerRecoveryQuarantined)?;
    let incarnation_id = lines[3]
        .strip_prefix("installation_incarnation_id=")
        .ok_or(OwnerError::OwnerRecoveryQuarantined)?;
    Ok(InstallationBinding {
        installation_id: parse_id16(Some(installation_id))?,
        installation_incarnation_id: parse_id16(Some(incarnation_id))?,
    })
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
    };
    let mut text = String::new();
    push_line(&mut text, INSTALLATION_MAGIC);
    push_line(&mut text, FORMAT_VERSION_LINE);
    push_field(
        &mut text,
        "installation_id",
        &hex(&binding.installation_id),
    );
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
    fs::read(path).map_or(Err(OwnerError::OwnerAcquireOutcomeUnknown), |bytes| {
        parse_installation(&bytes)
    })
}
