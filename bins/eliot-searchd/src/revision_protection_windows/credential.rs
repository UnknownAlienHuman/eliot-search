//! Credential Manager root-secret lifecycle and readback verification.

use core::ptr::null_mut;
use std::path::Path;

use crate::sha256;

use super::ffi::{
    BCRYPT_USE_SYSTEM_PREFERRED_RNG, CRED_PERSIST_LOCAL_MACHINE,
    CRED_TYPE_GENERIC, CredentialAllocation, CredentialW, ERROR_NOT_FOUND,
    FileTime, ROOT_SECRET_BYTES, acquire_vault_lock, bcrypt_gen_random,
    cred_read_w, cred_write_w, get_last_error, wide,
};
use super::inventory::contains_protected_objects;

pub(super) fn load_or_create_root_secret(
    namespace_id: [u8; 32],
    revision_root: &Path,
) -> Result<[u8; 32], String> {
    let target = wide(&format!(
        "ELIOT Search/revision-key/{}",
        sha256::hex(&namespace_id),
    ));
    if let Some(secret) = read_credential(&target)? {
        return Ok(secret);
    }
    if contains_protected_objects(revision_root)? {
        for attempt in 0..8_u32 {
            std::thread::sleep(core::time::Duration::from_millis(
                10_u64 << attempt.min(5),
            ));
            let Ok(_lock) = acquire_vault_lock() else {
                continue;
            };
            if let Some(secret) = read_credential(&target)? {
                return Ok(secret);
            }
        }
        return Err("DIRECT_REVISION_KEY_MISSING".to_owned());
    }

    let mut secret = [0_u8; ROOT_SECRET_BYTES];
    let status = unsafe {
        bcrypt_gen_random(
            null_mut(),
            secret.as_mut_ptr(),
            u32::try_from(secret.len())
                .map_err(|_| "DIRECT_REVISION_RNG_FAILED".to_owned())?,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status < 0 {
        super::super::zeroize(&mut secret);
        return Err(format!("DIRECT_REVISION_RNG_FAILED:{status}"));
    }
    for attempt in 0..8_u32 {
        if attempt > 0 {
            std::thread::sleep(core::time::Duration::from_millis(
                10_u64 << attempt.min(5),
            ));
        }
        let Ok(_lock) = acquire_vault_lock() else {
            continue;
        };
        if let Err(error) = write_credential(&target, &mut secret) {
            if !is_transient_vault_outcome(&error) {
                super::super::zeroize(&mut secret);
                return Err(error);
            }
            continue;
        }
        match read_credential(&target) {
            Ok(Some(observed)) => {
                if !constant_time_equal(&secret, &observed) {
                    super::super::zeroize(&mut secret);
                    let mut observed = observed;
                    super::super::zeroize(&mut observed);
                    return Err("DIRECT_REVISION_KEY_READBACK_MISMATCH".to_owned());
                }
                let mut observed = observed;
                super::super::zeroize(&mut observed);
                return Ok(secret);
            }
            Ok(None) => {}
            Err(error) => {
                if !is_transient_vault_outcome(&error) {
                    super::super::zeroize(&mut secret);
                    return Err(error);
                }
            }
        }
    }
    super::super::zeroize(&mut secret);
    Err("DIRECT_REVISION_KEY_WRITE_OUTCOME_UNKNOWN".to_owned())
}

fn is_transient_vault_outcome(error: &str) -> bool {
    error.starts_with("DIRECT_REVISION_KEY_WRITE_FAILED")
        || error == "DIRECT_REVISION_KEY_WRITE_OUTCOME_UNKNOWN"
        || error.starts_with("DIRECT_REVISION_KEY_READ_FAILED")
}

pub(super) fn read_credential(
    target: &[u16],
) -> Result<Option<[u8; 32]>, String> {
    let mut pointer = null_mut::<CredentialW>();
    let success = unsafe {
        cred_read_w(
            target.as_ptr(),
            CRED_TYPE_GENERIC,
            0,
            &raw mut pointer,
        )
    };
    if success == 0 {
        let error = unsafe { get_last_error() };
        return if error == ERROR_NOT_FOUND {
            Ok(None)
        } else {
            Err(format!("DIRECT_REVISION_KEY_READ_FAILED:{error}"))
        };
    }
    if pointer.is_null() {
        return Err("DIRECT_REVISION_KEY_READBACK_INVALID".to_owned());
    }
    let allocation = CredentialAllocation(pointer);
    let credential = unsafe { &*allocation.0 };
    let size = usize::try_from(credential.credential_blob_size)
        .map_err(|_| "DIRECT_REVISION_KEY_READBACK_INVALID".to_owned())?;
    if credential.credential_type != CRED_TYPE_GENERIC
        || credential.persist != CRED_PERSIST_LOCAL_MACHINE
        || size != ROOT_SECRET_BYTES
        || credential.credential_blob.is_null()
    {
        return Err("DIRECT_REVISION_KEY_READBACK_INVALID".to_owned());
    }
    let mut secret = [0_u8; ROOT_SECRET_BYTES];
    unsafe {
        core::ptr::copy_nonoverlapping(
            credential.credential_blob,
            secret.as_mut_ptr(),
            ROOT_SECRET_BYTES,
        );
    }
    drop(allocation);
    Ok(Some(secret))
}

fn write_credential(
    target: &[u16],
    secret: &mut [u8; 32],
) -> Result<(), String> {
    let mut target = target.to_vec();
    let mut user_name = wide("ELIOT Search");
    let credential = CredentialW {
        flags: 0,
        credential_type: CRED_TYPE_GENERIC,
        target_name: target.as_mut_ptr(),
        comment: null_mut(),
        last_written: FileTime {
            low_date_time: 0,
            high_date_time: 0,
        },
        credential_blob_size: u32::try_from(secret.len())
            .map_err(|_| "DIRECT_REVISION_KEY_TOO_LARGE".to_owned())?,
        credential_blob: secret.as_mut_ptr(),
        persist: CRED_PERSIST_LOCAL_MACHINE,
        attribute_count: 0,
        attributes: null_mut(),
        target_alias: null_mut(),
        user_name: user_name.as_mut_ptr(),
    };
    let success = unsafe { cred_write_w(&raw const credential, 0) };
    if success == 0 {
        let error = unsafe { get_last_error() };
        return Err(format!("DIRECT_REVISION_KEY_WRITE_FAILED:{error}"));
    }
    Ok(())
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0_u8;
    for (left, right) in left.iter().zip(right.iter()) {
        difference |= left ^ right;
    }
    difference == 0
}
