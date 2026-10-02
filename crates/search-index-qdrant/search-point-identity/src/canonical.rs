use crate::{
    PointIdentityError, PointIdentityLimits, ProjectionPointKey,
};

const MAP_FIELDS: u64 = 8;

// RFC 8949 deterministic map order: shorter encoded keys first, then bytewise.
const KEY_UNIT_ID: &str = "unit_id";
const KEY_POINT_ROLE: &str = "point_role";
const KEY_SCHEMA_VERSION: &str = "schema_version";
const KEY_REPRESENTATION_ID: &str = "representation_id";
const KEY_COLLECTION_GENERATION_ID: &str = "collection_generation_id";
const KEY_PROJECTION_MEMBERSHIP_ID: &str = "projection_membership_id";
const KEY_PROJECTION_PROFILE_SET_ID: &str = "projection_profile_set_id";
const KEY_INSTALLATION_INCARNATION_ID: &str = "installation_incarnation_id";

/// Bounded deterministic CBOR bytes for one point key.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalPointKeyBytes(Vec<u8>);

impl CanonicalPointKeyBytes {
    /// Borrows the exact canonical bytes.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    /// Consumes the wrapper and returns the exact bytes.
    #[must_use]
    pub fn into_vec(self) -> Vec<u8> {
        self.0
    }
}

/// Encodes the exact S11.1 key as deterministic canonical CBOR.
///
/// UUID identities are encoded as 16-byte byte strings, the profile-set ID and
/// point role as UTF-8 text, and the schema version as an unsigned integer.
/// Unknown fields, omitted fields, JSON and map-iteration order are
/// unrepresentable.
pub fn encode_canonical_key(
    key: &ProjectionPointKey,
    limits: PointIdentityLimits,
) -> Result<CanonicalPointKeyBytes, PointIdentityError> {
    key.validate(limits)?;
    let limits = limits.validate()?;
    let mut output = Vec::with_capacity(320);
    write_type_and_length(&mut output, 5, MAP_FIELDS, limits)?;

    write_text(&mut output, KEY_UNIT_ID, limits)?;
    write_bytes(&mut output, key.unit_id.as_bytes(), limits)?;

    write_text(&mut output, KEY_POINT_ROLE, limits)?;
    write_text(&mut output, key.point_role.as_str(), limits)?;

    write_text(&mut output, KEY_SCHEMA_VERSION, limits)?;
    write_unsigned(&mut output, u64::from(key.schema_version), limits)?;

    write_text(&mut output, KEY_REPRESENTATION_ID, limits)?;
    write_bytes(&mut output, key.representation_id.as_bytes(), limits)?;

    write_text(&mut output, KEY_COLLECTION_GENERATION_ID, limits)?;
    write_bytes(
        &mut output,
        key.collection_generation_id.as_bytes(),
        limits,
    )?;

    write_text(&mut output, KEY_PROJECTION_MEMBERSHIP_ID, limits)?;
    write_bytes(
        &mut output,
        key.projection_membership_id.as_bytes(),
        limits,
    )?;

    write_text(&mut output, KEY_PROJECTION_PROFILE_SET_ID, limits)?;
    write_text(
        &mut output,
        key.projection_profile_set_id.as_str(),
        limits,
    )?;

    write_text(&mut output, KEY_INSTALLATION_INCARNATION_ID, limits)?;
    write_bytes(
        &mut output,
        key.installation_incarnation_id.as_bytes(),
        limits,
    )?;

    Ok(CanonicalPointKeyBytes(output))
}

/// Agent-contract spelling for [`encode_canonical_key`].
pub fn canonical_point_key_bytes(
    key: &ProjectionPointKey,
    limits: PointIdentityLimits,
) -> Result<CanonicalPointKeyBytes, PointIdentityError> {
    encode_canonical_key(key, limits)
}

fn write_unsigned(
    output: &mut Vec<u8>,
    value: u64,
    limits: PointIdentityLimits,
) -> Result<(), PointIdentityError> {
    write_type_and_length(output, 0, value, limits)
}

fn write_bytes(
    output: &mut Vec<u8>,
    value: &[u8],
    limits: PointIdentityLimits,
) -> Result<(), PointIdentityError> {
    let length = u64::try_from(value.len())
        .map_err(|_| PointIdentityError::CanonicalEncodingFailed)?;
    write_type_and_length(output, 2, length, limits)?;
    extend_checked(output, value, limits)
}

fn write_text(
    output: &mut Vec<u8>,
    value: &str,
    limits: PointIdentityLimits,
) -> Result<(), PointIdentityError> {
    let bytes = value.as_bytes();
    let length = u64::try_from(bytes.len())
        .map_err(|_| PointIdentityError::CanonicalEncodingFailed)?;
    write_type_and_length(output, 3, length, limits)?;
    extend_checked(output, bytes, limits)
}

fn write_type_and_length(
    output: &mut Vec<u8>,
    major: u8,
    value: u64,
    limits: PointIdentityLimits,
) -> Result<(), PointIdentityError> {
    let prefix = major << 5;
    if value < 24 {
        let small = u8::try_from(value)
            .map_err(|_| PointIdentityError::CanonicalEncodingFailed)?;
        extend_checked(output, &[prefix | small], limits)
    } else if let Ok(number) = u8::try_from(value) {
        extend_checked(output, &[prefix | 0x18, number], limits)
    } else if let Ok(number) = u16::try_from(value) {
        extend_checked(output, &[prefix | 0x19], limits)?;
        extend_checked(output, &number.to_be_bytes(), limits)
    } else if let Ok(number) = u32::try_from(value) {
        extend_checked(output, &[prefix | 0x1a], limits)?;
        extend_checked(output, &number.to_be_bytes(), limits)
    } else {
        extend_checked(output, &[prefix | 0x1b], limits)?;
        extend_checked(output, &value.to_be_bytes(), limits)
    }
}

fn extend_checked(
    output: &mut Vec<u8>,
    value: &[u8],
    limits: PointIdentityLimits,
) -> Result<(), PointIdentityError> {
    let next = output
        .len()
        .checked_add(value.len())
        .ok_or(PointIdentityError::CanonicalEncodingFailed)?;
    if next > limits.max_canonical_bytes {
        return Err(PointIdentityError::CanonicalBytesExceeded);
    }
    output.extend_from_slice(value);
    Ok(())
}
