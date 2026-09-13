//! Declared-encoding and BOM admission.

use crate::MaterializationError;
use crate::profile::{BomPolicy, SourceEncoding, ValidatedMaterializerProfile};

use super::model::EncodingDecision;

pub(super) const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
pub(super) const UTF16LE_BOM: &[u8] = &[0xFF, 0xFE];
pub(super) const UTF16BE_BOM: &[u8] = &[0xFE, 0xFF];

/// Decides the encoding from the declared hint, the profile and BOM evidence.
///
/// A byte prefix that admits another encoding than the declared one is
/// [`MaterializationError::EncodingAmbiguous`]; a declared encoding outside
/// the profile set is [`MaterializationError::EncodingUnsupported`].
pub fn detect_or_validate_encoding(
    bytes: &[u8],
    declared: SourceEncoding,
    profile: &ValidatedMaterializerProfile,
) -> Result<EncodingDecision, MaterializationError> {
    if !profile.encodings().contains(&declared) {
        return Err(MaterializationError::EncodingUnsupported);
    }
    let sniffed = if bytes.starts_with(UTF8_BOM) {
        Some((SourceEncoding::Utf8, UTF8_BOM.len()))
    } else if bytes.starts_with(UTF16LE_BOM) {
        Some((SourceEncoding::Utf16Le, UTF16LE_BOM.len()))
    } else if bytes.starts_with(UTF16BE_BOM) {
        Some((SourceEncoding::Utf16Be, UTF16BE_BOM.len()))
    } else {
        None
    };
    let (encoding, bom_len_bytes) = match (declared, sniffed) {
        (SourceEncoding::Utf8, None) => (SourceEncoding::Utf8, 0),
        (SourceEncoding::Utf8, Some((SourceEncoding::Utf8, len))) => (SourceEncoding::Utf8, len),
        (SourceEncoding::Utf16Le, None) => (SourceEncoding::Utf16Le, 0),
        (SourceEncoding::Utf16Le, Some((SourceEncoding::Utf16Le, len))) => {
            (SourceEncoding::Utf16Le, len)
        }
        (SourceEncoding::Utf16Be, None) => (SourceEncoding::Utf16Be, 0),
        (SourceEncoding::Utf16Be, Some((SourceEncoding::Utf16Be, len))) => {
            (SourceEncoding::Utf16Be, len)
        }
        _ => return Err(MaterializationError::EncodingAmbiguous),
    };
    let bom_present = bom_len_bytes > 0;
    if bom_present && profile.bom_policy() == BomPolicy::RejectWhenPresent {
        return Err(MaterializationError::Unsupported);
    }
    Ok(EncodingDecision::new(
        encoding,
        bom_present,
        bom_len_bytes,
    ))
}
