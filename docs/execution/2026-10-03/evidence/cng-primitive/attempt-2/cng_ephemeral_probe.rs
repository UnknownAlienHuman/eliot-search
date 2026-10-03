//! Isolated CNG primitive diagnostic; never signs a control record or provisions an actor.
//! NULL key names and unnamed public imports keep both keys ephemeral.
use std::ffi::c_void;
use std::ptr::{null, null_mut};

const SILENT: u32 = 0x40;
const BAD_SIGNATURE: u32 = 0x8009_0006;

#[link(name = "ncrypt")]
unsafe extern "system" {
    fn NCryptOpenStorageProvider(out: *mut usize, name: *const u16, flags: u32) -> i32;
    fn NCryptCreatePersistedKey(provider: usize, out: *mut usize, algorithm: *const u16,
        name: *const u16, legacy: u32, flags: u32) -> i32;
    fn NCryptSetProperty(object: usize, property: *const u16, input: *mut u8,
        bytes: u32, flags: u32) -> i32;
    fn NCryptGetProperty(object: usize, property: *const u16, output: *mut u8,
        bytes: u32, result: *mut u32, flags: u32) -> i32;
    fn NCryptFinalizeKey(key: usize, flags: u32) -> i32;
    fn NCryptExportKey(key: usize, wrapping_key: usize, blob_type: *const u16,
        parameters: *mut c_void, output: *mut u8, bytes: u32,
        result: *mut u32, flags: u32) -> i32;
    fn NCryptImportKey(provider: usize, wrapping_key: usize, blob_type: *const u16,
        parameters: *mut c_void, out: *mut usize, data: *mut u8,
        bytes: u32, flags: u32) -> i32;
    fn NCryptSignHash(key: usize, padding: *mut c_void, digest: *mut u8,
        digest_bytes: u32, signature: *mut u8, signature_bytes: u32,
        result: *mut u32, flags: u32) -> i32;
    fn NCryptVerifySignature(key: usize, padding: *mut c_void, digest: *mut u8,
        digest_bytes: u32, signature: *mut u8, signature_bytes: u32, flags: u32) -> i32;
    fn NCryptFreeObject(object: usize) -> i32;
}

struct Object(usize);
impl Drop for Object {
    fn drop(&mut self) {
        // The provider owns all private material. No private-byte buffer exists here.
        let status = unsafe { NCryptFreeObject(self.0) };
        if status != 0 {
            eprintln!("NCryptFreeObject_status=0x{:08x}", status as u32);
        }
    }
}
fn wide(text: &str) -> Vec<u16> { text.encode_utf16().chain([0]).collect() }
fn ok(step: &str, status: i32) -> Result<(), String> {
    if status == 0 { Ok(()) } else { Err(format!("{step}:0x{:08x}", status as u32)) }
}
fn require(step: &str, condition: bool) -> Result<(), String> {
    if condition { Ok(()) } else { Err(step.to_owned()) }
}

fn run() -> Result<(), String> {
    require("windows_x64_required", cfg!(all(windows, target_arch = "x86_64")))?;
    let mut provider_handle = 0;
    let provider_name = wide("Microsoft Software Key Storage Provider");
    ok("open_provider", unsafe {
        NCryptOpenStorageProvider(&mut provider_handle, provider_name.as_ptr(), 0)
    })?;
    let provider = Object(provider_handle);
    let mut key_handle = 0;
    let algorithm = wide("ECDSA_P256");
    ok("create_unnamed_ephemeral_key", unsafe {
        NCryptCreatePersistedKey(provider.0, &mut key_handle, algorithm.as_ptr(), null(), 0, 0)
    })?;
    let key = Object(key_handle);
    let export_property = wide("Export Policy");
    let mut zero_policy = 0u32.to_le_bytes();
    ok("set_export_policy_zero", unsafe {
        NCryptSetProperty(key.0, export_property.as_ptr(), zero_policy.as_mut_ptr(), 4, 0)
    })?;
    ok("finalize_ephemeral_key", unsafe { NCryptFinalizeKey(key.0, SILENT) })?;
    let mut observed_policy = [0u8; 4];
    let mut observed_policy_bytes = 0;
    ok("read_export_policy", unsafe {
        NCryptGetProperty(key.0, export_property.as_ptr(), observed_policy.as_mut_ptr(),
            4, &mut observed_policy_bytes, 0)
    })?;
    require("export_policy_readback_mismatch", observed_policy_bytes == 4
        && u32::from_le_bytes(observed_policy) == 0)?;

    let public_blob_type = wide("ECCPUBLICBLOB");
    let mut public_blob = [0u8; 72];
    let mut public_bytes = 0;
    ok("export_public_key", unsafe {
        NCryptExportKey(key.0, 0, public_blob_type.as_ptr(), null_mut(),
            public_blob.as_mut_ptr(), 72, &mut public_bytes, SILENT)
    })?;
    require("public_blob_shape_mismatch", public_bytes == 72
        && u32::from_le_bytes(public_blob[0..4].try_into().unwrap()) == 0x3153_4345
        && u32::from_le_bytes(public_blob[4..8].try_into().unwrap()) == 32)?;
    let mut public_handle = 0;
    ok("import_unnamed_public_key", unsafe {
        NCryptImportKey(provider.0, 0, public_blob_type.as_ptr(), null_mut(),
            &mut public_handle, public_blob.as_mut_ptr(), 72, SILENT)
    })?;
    let public_key = Object(public_handle);

    // SHA-256 of an empty diagnostic message. This is not an approval preimage.
    let mut digest = [0xe3,0xb0,0xc4,0x42,0x98,0xfc,0x1c,0x14,0x9a,0xfb,0xf4,0xc8,0x99,0x6f,0xb9,0x24,
        0x27,0xae,0x41,0xe4,0x64,0x9b,0x93,0x4c,0xa4,0x95,0x99,0x1b,0x78,0x52,0xb8,0x55];
    let mut signature = [0u8; 64];
    let mut signature_bytes = 0;
    ok("sign_diagnostic_digest", unsafe {
        NCryptSignHash(key.0, null_mut(), digest.as_mut_ptr(), 32,
            signature.as_mut_ptr(), 64, &mut signature_bytes, SILENT)
    })?;
    require("unexpected_signature_length", signature_bytes == 64)?;
    ok("verify_with_imported_public_key", unsafe {
        NCryptVerifySignature(public_key.0, null_mut(), digest.as_mut_ptr(), 32,
            signature.as_mut_ptr(), 64, SILENT)
    })?;
    let mut changed_digest = digest;
    changed_digest[0] ^= 1;
    let changed_digest_status = unsafe {
        NCryptVerifySignature(public_key.0, null_mut(), changed_digest.as_mut_ptr(), 32,
            signature.as_mut_ptr(), 64, SILENT)
    } as u32;
    require("changed_digest_not_bad_signature", changed_digest_status == BAD_SIGNATURE)?;
    let mut changed_signature = signature;
    changed_signature[0] ^= 1;
    let changed_signature_status = unsafe {
        NCryptVerifySignature(public_key.0, null_mut(), digest.as_mut_ptr(), 32,
            changed_signature.as_mut_ptr(), 64, SILENT)
    } as u32;
    require("changed_signature_not_bad_signature", changed_signature_status == BAD_SIGNATURE)?;

    // Size query only: no private output buffer exists. Export Policy is documented for
    // persisted keys, so this ephemeral observation cannot prove persisted-key denial.
    let private_blob_type = wide("ECCPRIVATEBLOB");
    let mut private_size_result = 0;
    let private_export_status = unsafe {
        NCryptExportKey(key.0, 0, private_blob_type.as_ptr(), null_mut(), null_mut(),
            0, &mut private_size_result, SILENT)
    } as u32;
    println!("{{\"scope\":\"CNG_EPHEMERAL_PRIMITIVE_DIAGNOSTIC\",\"authority\":\"NON_AUTHORITATIVE\",\"provider\":\"Microsoft Software Key Storage Provider\",\"key_name_supplied\":false,\"actor_provisioned\":false,\"profile_qualified\":false,\"export_policy_readback\":0,\"public_blob_bytes\":72,\"signature_bytes\":64,\"imported_public_verification_succeeded\":true,\"changed_digest_status\":\"0x{changed_digest_status:08x}\",\"changed_signature_status\":\"0x{changed_signature_status:08x}\",\"private_size_query_status\":\"0x{private_export_status:08x}\",\"private_size_query_result\":{private_size_result},\"private_export_denial_verified\":false,\"private_bytes_requested\":0}}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("CNG_DIAGNOSTIC_FAILED:{error}");
        std::process::exit(1);
    }
}
