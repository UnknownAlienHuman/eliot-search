//! Closed S11 field decoding through the shared canonical parser.

use search_contracts::{
    CanonicalValue, ClosedCanonicalObject, CollectionGenerationId, InstallationIncarnationId,
    ProjectionMembershipId, ProjectionProfileSetId, RepresentationId, UnitId, parse_canonical_cbor,
};

use super::{PointIdError, PointIdentityLimits, PointRole, ProjectionPointKey};

impl ProjectionPointKey {
    /// Decodes exactly eight fields from bounded canonical CBOR.
    ///
    /// Rejects legacy bytes, unknown fields/versions/roles, noncanonical bytes
    /// and incorrectly typed UUIDs before returning a validated S11 key.
    pub fn from_canonical_bytes(
        bytes: &[u8],
        limits: PointIdentityLimits,
    ) -> Result<Self, PointIdError> {
        let limits = limits.validate()?;
        if bytes.len() > limits.max_canonical_bytes {
            return Err(PointIdError::CanonicalBytesExceeded);
        }
        let value =
            parse_canonical_cbor(bytes).map_err(|_| PointIdError::CanonicalEncodingMismatch)?;
        let mut fields = ClosedCanonicalObject::from_value(value, "s11_point_key")
            .map_err(|_| PointIdError::CanonicalEncodingMismatch)?;
        let schema_version = match take(&mut fields, "schema_version")? {
            CanonicalValue::U64(value) => {
                u16::try_from(value).map_err(|_| PointIdError::PointKeyVersionUnsupported)?
            }
            _ => return Err(PointIdError::CanonicalEncodingMismatch),
        };
        let key = Self {
            schema_version,
            installation_incarnation_id: InstallationIncarnationId::from_bytes(uuid(
                &mut fields,
                "installation_incarnation_id",
            )?),
            collection_generation_id: CollectionGenerationId::from_bytes(uuid(
                &mut fields,
                "collection_generation_id",
            )?),
            projection_membership_id: ProjectionMembershipId::from_bytes(uuid(
                &mut fields,
                "projection_membership_id",
            )?),
            representation_id: RepresentationId::from_bytes(uuid(
                &mut fields,
                "representation_id",
            )?),
            unit_id: UnitId::from_bytes(uuid(&mut fields, "unit_id")?),
            projection_profile_set_id: ProjectionProfileSetId::new(text(
                &mut fields,
                "projection_profile_set_id",
            )?)
            .map_err(|_| PointIdError::CanonicalEncodingMismatch)?,
            point_role: PointRole::parse(&text(&mut fields, "point_role")?)?,
        };
        fields
            .finish()
            .map_err(|_| PointIdError::CanonicalEncodingMismatch)?;
        key.validate(limits)?;
        Ok(key)
    }
}

fn take(
    fields: &mut ClosedCanonicalObject,
    name: &'static str,
) -> Result<CanonicalValue, PointIdError> {
    fields
        .take_required(name)
        .map_err(|_| PointIdError::CanonicalEncodingMismatch)
}

fn uuid(fields: &mut ClosedCanonicalObject, name: &'static str) -> Result<[u8; 16], PointIdError> {
    match take(fields, name)? {
        CanonicalValue::Bytes(bytes) => bytes
            .as_slice()
            .try_into()
            .map_err(|_| PointIdError::CanonicalEncodingMismatch),
        _ => Err(PointIdError::CanonicalEncodingMismatch),
    }
}

fn text(fields: &mut ClosedCanonicalObject, name: &'static str) -> Result<String, PointIdError> {
    match take(fields, name)? {
        CanonicalValue::Text(value) => Ok(value.into_string()),
        _ => Err(PointIdError::CanonicalEncodingMismatch),
    }
}
