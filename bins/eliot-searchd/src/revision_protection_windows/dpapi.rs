//! DPAPI protect/unprotect translation with zeroized native outputs.

use core::ptr::{null, null_mut};

use super::ffi::{
    CRYPTPROTECT_UI_FORBIDDEN, DataBlob, LocalAllocation,
    crypt_protect_data, crypt_unprotect_data, get_last_error,
};

pub(super) fn protect_data(
    input: &mut [u8],
    entropy: &[u8; 32],
) -> Result<Vec<u8>, String> {
    let input_len = u32::try_from(input.len())
        .map_err(|_| "DIRECT_DPAPI_INPUT_TOO_LARGE".to_owned())?;
    let mut entropy_copy = *entropy;
    let input_blob = DataBlob {
        byte_length: input_len,
        bytes: input.as_mut_ptr(),
    };
    let entropy_blob = DataBlob {
        byte_length: u32::try_from(entropy_copy.len())
            .map_err(|_| "DIRECT_DPAPI_INPUT_TOO_LARGE".to_owned())?,
        bytes: entropy_copy.as_mut_ptr(),
    };
    let mut output = DataBlob {
        byte_length: 0,
        bytes: null_mut(),
    };
    let success = unsafe {
        crypt_protect_data(
            &raw const input_blob,
            null(),
            &raw const entropy_blob,
            null_mut(),
            null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &raw mut output,
        )
    };
    super::super::zeroize(&mut entropy_copy);
    if success == 0 {
        let error = unsafe { get_last_error() };
        return Err(format!("DIRECT_DPAPI_PROTECT_FAILED:{error}"));
    }
    LocalAllocation(output).into_vec(super::super::MAX_PROTECTED_OBJECT_BYTES)
}

pub(super) fn unprotect_data(
    input: &mut [u8],
    entropy: &[u8; 32],
) -> Result<Vec<u8>, String> {
    let input_len = u32::try_from(input.len())
        .map_err(|_| "DIRECT_DPAPI_INPUT_TOO_LARGE".to_owned())?;
    let mut entropy_copy = *entropy;
    let input_blob = DataBlob {
        byte_length: input_len,
        bytes: input.as_mut_ptr(),
    };
    let entropy_blob = DataBlob {
        byte_length: u32::try_from(entropy_copy.len())
            .map_err(|_| "DIRECT_DPAPI_INPUT_TOO_LARGE".to_owned())?,
        bytes: entropy_copy.as_mut_ptr(),
    };
    let mut output = DataBlob {
        byte_length: 0,
        bytes: null_mut(),
    };
    let success = unsafe {
        crypt_unprotect_data(
            &raw const input_blob,
            null_mut(),
            &raw const entropy_blob,
            null_mut(),
            null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &raw mut output,
        )
    };
    super::super::zeroize(&mut entropy_copy);
    if success == 0 {
        let error = unsafe { get_last_error() };
        return Err(format!("DIRECT_DPAPI_UNPROTECT_FAILED:{error}"));
    }
    LocalAllocation(output).into_vec(super::super::MAX_PROTECTED_OBJECT_BYTES)
}
