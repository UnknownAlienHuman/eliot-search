//! Canonical standalone-grant request decoding.

use search_contracts::{
    AccessPartitionId, BoundedSet, DisclosureCeiling, Modality, ProfileId, RecipeIdV1,
    SensitivityClass, SourceMembershipId, MAX_PROFILE_ID_BYTES,
};

use crate::error::ProtocolError;

use super::cursor::Cursor;
use super::encode::encode_standalone_grant_request;
use super::super::{
    MAX_STANDALONE_GRANT_REQUEST_BYTES, STANDALONE_GRANT_REQUEST_VERSION,
    StandaloneGrantRequestV1,
};

/// Decodes one exact canonical standalone-grant request body.
///
/// The parser accepts no whitespace, unknown fields, alternate field order,
/// escaped strings, duplicate set members, upper-case hex, leading-zero
/// numbers or trailing bytes. Parsed values are re-encoded and compared with
/// the input to enforce one canonical representation.
///
/// # Errors
///
/// Returns a typed protocol error for malformed, non-canonical or oversized
/// bodies.
pub fn decode_standalone_grant_request(
    body: &[u8],
) -> Result<StandaloneGrantRequestV1, ProtocolError> {
    if body.is_empty() || body.len() > MAX_STANDALONE_GRANT_REQUEST_BYTES {
        return Err(if body.len() > MAX_STANDALONE_GRANT_REQUEST_BYTES {
            ProtocolError::FrameTooLarge
        } else {
            ProtocolError::InvalidBody
        });
    }
    let mut cursor = Cursor::new(body);
    cursor.expect(b"{\"v\":")?;
    let version = cursor.parse_u64()?;
    if version != u64::from(STANDALONE_GRANT_REQUEST_VERSION) {
        return Err(ProtocolError::InvalidVersion);
    }
    cursor.expect(b",\"binding_generation\":")?;
    let expected_binding_generation = cursor.parse_u64()?;
    cursor.expect(b",\"policy_generation\":")?;
    let expected_policy_generation = cursor.parse_u64()?;
    cursor.expect(b",\"memberships\":")?;
    let memberships = cursor.parse_array(|cursor| {
        cursor.parse_uuid(SourceMembershipId::from_bytes)
    })?;
    cursor.expect(b",\"targets\":")?;
    let targets = cursor.parse_array(|cursor| cursor.parse_target())?;
    cursor.expect(b",\"partitions\":")?;
    let partitions = cursor.parse_array(|cursor| {
        cursor.parse_uuid(AccessPartitionId::from_bytes)
    })?;
    cursor.expect(b",\"modalities\":")?;
    let modalities = cursor.parse_array(|cursor| {
        let token = cursor.parse_quoted_token(32)?;
        let token = core::str::from_utf8(token).map_err(|_| ProtocolError::InvalidBody)?;
        Modality::parse(token).map_err(|_| ProtocolError::InvalidBody)
    })?;
    cursor.expect(b",\"recipes\":")?;
    let recipes = cursor.parse_array(|cursor| {
        let token = cursor.parse_quoted_token(64)?;
        let token = core::str::from_utf8(token).map_err(|_| ProtocolError::InvalidBody)?;
        RecipeIdV1::parse_versioned(token).map_err(|_| ProtocolError::InvalidBody)
    })?;
    cursor.expect(b",\"budget_hex\":\"")?;
    let budget_bytes = cursor.parse_hex_bytes(MAX_PROFILE_ID_BYTES)?;
    cursor.expect(b"\",\"sensitivity\":\"")?;
    let sensitivity = cursor.parse_token_until_quote(32)?;
    let sensitivity = core::str::from_utf8(sensitivity).map_err(|_| ProtocolError::InvalidBody)?;
    let requested_sensitivity_ceiling =
        SensitivityClass::parse(sensitivity).map_err(|_| ProtocolError::InvalidBody)?;
    cursor.expect(b"\",\"disclosure\":\"")?;
    let disclosure = cursor.parse_token_until_quote(32)?;
    let disclosure = core::str::from_utf8(disclosure).map_err(|_| ProtocolError::InvalidBody)?;
    let requested_disclosure_ceiling =
        DisclosureCeiling::parse(disclosure).map_err(|_| ProtocolError::InvalidBody)?;
    cursor.expect(b"\",\"source_read\":")?;
    let requested_source_read_permission = cursor.parse_bool()?;
    cursor.expect(b",\"exact_scan\":")?;
    let requested_exact_scan_permission = cursor.parse_bool()?;
    cursor.expect(b",\"ttl_ms\":")?;
    let requested_ttl_ms = cursor.parse_u64()?;
    cursor.expect(b"}")?;
    if !cursor.is_exhausted() {
        return Err(ProtocolError::InvalidBody);
    }

    let budget = String::from_utf8(budget_bytes).map_err(|_| ProtocolError::InvalidBody)?;
    let request = StandaloneGrantRequestV1 {
        expected_binding_generation,
        expected_policy_generation,
        requested_membership_ids: BoundedSet::from_items(memberships)
            .map_err(|_| ProtocolError::InvalidBody)?,
        requested_corpus_or_portfolio_ids: BoundedSet::from_items(targets)
            .map_err(|_| ProtocolError::InvalidBody)?,
        requested_access_partitions: BoundedSet::from_items(partitions)
            .map_err(|_| ProtocolError::InvalidBody)?,
        requested_modalities: BoundedSet::from_items(modalities)
            .map_err(|_| ProtocolError::InvalidBody)?,
        requested_recipe_families: BoundedSet::from_items(recipes)
            .map_err(|_| ProtocolError::InvalidBody)?,
        requested_budget_class: ProfileId::new(budget).map_err(|_| ProtocolError::InvalidBody)?,
        requested_sensitivity_ceiling,
        requested_disclosure_ceiling,
        requested_source_read_permission,
        requested_exact_scan_permission,
        requested_ttl_ms,
    };
    request.validate()?;
    if encode_standalone_grant_request(&request)?.as_slice() != body {
        return Err(ProtocolError::InvalidBody);
    }
    Ok(request)
}
