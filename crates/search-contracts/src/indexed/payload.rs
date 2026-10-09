//! Typed S9.5 point payload: 20 closed fields, strict canonical round trip and
//! one internal BLAKE3 identity computed through the #237 helpers.

use crate::canonical::{BoundedName, BoundedSymbolKey};
use crate::ids::{
    AccessPartitionId, Blake3Digest32, CollectionGenerationId, InstallationIncarnationId,
    ProjectionMembershipId, ProjectionProfileSetId, RepositoryLineageId, RepresentationId,
    ScoringDocumentId, ScoringPartitionId, SourceId, SourceRevisionId, UnitId,
};
use crate::schema::{EntityKind, Modality};
use crate::source::UnitKind;
use crate::{
    CanonicalBytes, CanonicalValue, ClosedCanonicalObject, ContractError, ContractErrorKind, Epoch,
    to_canonical_cbor,
};

use super::codec::{decode_text, error, object, text};

/// Domain for the internal payload identity. CBOR is the only representation.
const PAYLOAD_IDENTITY_DOMAIN: &str = "eliot/cbor/indexed-payload/v1";

/// Canonical scalar encoding of one closed payload.
///
/// Source membership, access subjects, paths, source/query text, payload digest,
/// vector digest and vendor metadata are structurally absent by construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointPayload {
    /// Uuid, never a raw caller string.
    pub installation_incarnation_id: InstallationIncarnationId,
    /// Uuid, never a raw caller string.
    pub collection_generation_id: CollectionGenerationId,
    /// Uuid, never a raw caller string.
    pub projection_membership_id: ProjectionMembershipId,
    /// Uuid, never a raw caller string.
    pub access_partition_id: AccessPartitionId,
    /// Uuid, never a raw caller string.
    pub scoring_partition_id: ScoringPartitionId,
    /// Uuid, never a raw caller string.
    pub source_id: SourceId,
    /// Uuid, never a raw caller string.
    pub source_revision_id: SourceRevisionId,
    /// Uuid, never a raw caller string.
    pub representation_id: RepresentationId,
    /// Uuid, never a raw caller string.
    pub unit_id: UnitId,
    /// Readback evidence, never a caller-selected schema identity.
    pub point_identity_digest_256: Blake3Digest32,
    /// Uuid, never a raw caller string.
    pub scoring_document_id: ScoringDocumentId,
    /// Closed keyword profile identity.
    pub projection_profile_set_id: ProjectionProfileSetId,
    /// Closed wire enum.
    pub unit_kind: UnitKind,
    /// Closed wire enum.
    pub modality: Modality,
    /// Bounded keyword; not a path or source text.
    pub language_or_format: BoundedName,
    /// Optional closed wire enum; absent, never null.
    pub entity_kind: Option<EntityKind>,
    /// Optional bounded keyword; absent, never null.
    pub normalized_symbol_key: Option<BoundedSymbolKey>,
    /// Optional lineage uuid; absent, never null.
    pub repository_lineage_id: Option<RepositoryLineageId>,
    /// At least 1; zero is the empty initial generation.
    pub valid_from_epoch: Epoch,
    /// Exclusive upper bound, strictly greater than `valid_from_epoch`.
    pub valid_until_epoch_exclusive: Option<Epoch>,
}

impl PointPayload {
    /// Constructs one payload from exactly the closed field set.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        installation_incarnation_id: InstallationIncarnationId,
        collection_generation_id: CollectionGenerationId,
        projection_membership_id: ProjectionMembershipId,
        access_partition_id: AccessPartitionId,
        scoring_partition_id: ScoringPartitionId,
        source_id: SourceId,
        source_revision_id: SourceRevisionId,
        representation_id: RepresentationId,
        unit_id: UnitId,
        point_identity_digest_256: Blake3Digest32,
        scoring_document_id: ScoringDocumentId,
        projection_profile_set_id: ProjectionProfileSetId,
        unit_kind: UnitKind,
        modality: Modality,
        language_or_format: BoundedName,
        entity_kind: Option<EntityKind>,
        normalized_symbol_key: Option<BoundedSymbolKey>,
        repository_lineage_id: Option<RepositoryLineageId>,
        valid_from_epoch: Epoch,
        valid_until_epoch_exclusive: Option<Epoch>,
    ) -> Self {
        Self {
            installation_incarnation_id,
            collection_generation_id,
            projection_membership_id,
            access_partition_id,
            scoring_partition_id,
            source_id,
            source_revision_id,
            representation_id,
            unit_id,
            point_identity_digest_256,
            scoring_document_id,
            projection_profile_set_id,
            unit_kind,
            modality,
            language_or_format,
            entity_kind,
            normalized_symbol_key,
            repository_lineage_id,
            valid_from_epoch,
            valid_until_epoch_exclusive,
        }
    }

    /// Zero is the empty initial generation, never a published point.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.valid_from_epoch.get() < 1 {
            return Err(error(ContractErrorKind::InvalidRange, "valid_from_epoch"));
        }
        if let Some(until) = self.valid_until_epoch_exclusive
            && until <= self.valid_from_epoch
        {
            return Err(error(
                ContractErrorKind::InvalidRange,
                "valid_until_epoch_exclusive",
            ));
        }
        Ok(())
    }

    /// Canonical object over exactly the declared fields; optionals are absent.
    pub fn to_canonical_value(&self) -> Result<CanonicalValue, ContractError> {
        self.validate()?;
        let mut fields: Vec<(&'static str, CanonicalValue)> = Vec::with_capacity(20);
        fields.push((
            "installation_incarnation_id",
            text(&self.installation_incarnation_id.to_string())?,
        ));
        fields.push((
            "collection_generation_id",
            text(&self.collection_generation_id.to_string())?,
        ));
        fields.push((
            "projection_membership_id",
            text(&self.projection_membership_id.to_string())?,
        ));
        fields.push((
            "access_partition_id",
            text(&self.access_partition_id.to_string())?,
        ));
        fields.push((
            "scoring_partition_id",
            text(&self.scoring_partition_id.to_string())?,
        ));
        fields.push(("source_id", text(&self.source_id.to_string())?));
        fields.push((
            "source_revision_id",
            text(&self.source_revision_id.to_string())?,
        ));
        fields.push((
            "representation_id",
            text(&self.representation_id.to_string())?,
        ));
        fields.push(("unit_id", text(&self.unit_id.to_string())?));
        fields.push((
            "point_identity_digest_256",
            text(&self.point_identity_digest_256.to_string())?,
        ));
        fields.push((
            "scoring_document_id",
            text(&self.scoring_document_id.to_string())?,
        ));
        fields.push((
            "projection_profile_set_id",
            text(self.projection_profile_set_id.as_str())?,
        ));
        fields.push(("unit_kind", text(self.unit_kind.as_str())?));
        fields.push(("modality", text(self.modality.as_str())?));
        fields.push((
            "language_or_format",
            text(self.language_or_format.as_str())?,
        ));
        if let Some(entity_kind) = &self.entity_kind {
            fields.push(("entity_kind", text(entity_kind.as_str())?));
        }
        if let Some(key) = &self.normalized_symbol_key {
            fields.push(("normalized_symbol_key", text(key.as_str())?));
        }
        if let Some(lineage) = &self.repository_lineage_id {
            fields.push(("repository_lineage_id", text(&lineage.to_string())?));
        }
        fields.push((
            "valid_from_epoch",
            epoch_value(self.valid_from_epoch, "valid_from_epoch")?,
        ));
        if let Some(until) = self.valid_until_epoch_exclusive {
            fields.push((
                "valid_until_epoch_exclusive",
                epoch_value(until, "valid_until_epoch_exclusive")?,
            ));
        }
        object(fields)
    }

    /// Encode through the sole #237 canonical encoder.
    pub fn to_canonical_cbor(&self) -> Result<CanonicalBytes, ContractError> {
        to_canonical_cbor(&self.to_canonical_value()?)
    }

    /// Restores one payload from its closed canonical object.
    pub fn from_canonical_value(value: CanonicalValue) -> Result<Self, ContractError> {
        let payload = decode(value)?;
        payload.validate()?;
        Ok(payload)
    }

    /// Restores one payload from exact canonical CBOR bytes.
    pub fn from_canonical_cbor(bytes: &[u8]) -> Result<Self, ContractError> {
        let value = crate::parse_canonical_cbor(bytes)?;
        Self::from_canonical_value(value)
    }

    /// Internal schema identity, computed only through the #237 helpers.
    ///
    /// No caller-supplied identity is accepted and none is stored on the value,
    /// so a recovered payload always recomputes the same digest.
    pub fn identity(&self) -> Result<Blake3Digest32, ContractError> {
        let domain =
            crate::digest::CanonicalDigestDomain::parse(PAYLOAD_IDENTITY_DOMAIN).map_err(|_| {
                error(
                    ContractErrorKind::InvalidCharacter,
                    "payload_identity_domain",
                )
            })?;
        let limit = crate::digest::DigestInputLimit::new(crate::MAX_CANONICAL_BYTES)
            .map_err(|_| error(ContractErrorKind::InvalidRange, "payload_identity_limit"))?;
        crate::digest::blake3_canonical(&domain, &self.to_canonical_value()?, limit)
    }
}

/// Non-negative epoch as canonical `U64`; never a non-negative `I64`.
fn epoch_value(epoch: Epoch, field: &'static str) -> Result<CanonicalValue, ContractError> {
    let value =
        u64::try_from(epoch.get()).map_err(|_| error(ContractErrorKind::EpochOutOfRange, field))?;
    Ok(CanonicalValue::U64(value))
}

fn decode_text_field(
    fields: &mut ClosedCanonicalObject,
    field: &'static str,
) -> Result<String, ContractError> {
    decode_text(fields.take_required(field)?, field)
}

fn take_optional_text(
    fields: &mut ClosedCanonicalObject,
    field: &'static str,
) -> Result<Option<String>, ContractError> {
    fields
        .take_optional(field)?
        .map(|value| decode_text(value, field))
        .transpose()
}

fn take_uuid<T: std::str::FromStr>(
    fields: &mut ClosedCanonicalObject,
    field: &'static str,
) -> Result<T, ContractError> {
    decode_text_field(fields, field)?
        .parse::<T>()
        .map_err(|_| error(ContractErrorKind::InvalidCharacter, field))
}

fn take_optional_uuid<T: std::str::FromStr>(
    fields: &mut ClosedCanonicalObject,
    field: &'static str,
) -> Result<Option<T>, ContractError> {
    take_optional_text(fields, field)?
        .map(|value| value.parse::<T>())
        .transpose()
        .map_err(|_| error(ContractErrorKind::InvalidCharacter, field))
}

fn take_digest(
    fields: &mut ClosedCanonicalObject,
    field: &'static str,
) -> Result<Blake3Digest32, ContractError> {
    let text = decode_text_field(fields, field)?;
    Blake3Digest32::parse_hex(&text).map_err(|_| error(ContractErrorKind::InvalidDigest, field))
}

fn take_name(
    fields: &mut ClosedCanonicalObject,
    field: &'static str,
) -> Result<BoundedName, ContractError> {
    BoundedName::new(decode_text_field(fields, field)?)
}

fn take_enum<T>(
    fields: &mut ClosedCanonicalObject,
    field: &'static str,
    parse: fn(&str) -> Result<T, ContractError>,
) -> Result<T, ContractError> {
    parse(&decode_text_field(fields, field)?)
        .map_err(|_| error(ContractErrorKind::InvalidTaggedVariant, field))
}

fn take_optional_enum<T>(
    fields: &mut ClosedCanonicalObject,
    field: &'static str,
    parse: fn(&str) -> Result<T, ContractError>,
) -> Result<Option<T>, ContractError> {
    take_optional_text(fields, field)?
        .map(|value| parse(&value))
        .transpose()
        .map_err(|_| error(ContractErrorKind::InvalidTaggedVariant, field))
}

fn take_epoch(
    fields: &mut ClosedCanonicalObject,
    field: &'static str,
) -> Result<Epoch, ContractError> {
    super::codec::epoch(fields.take_required(field)?, field)
}

fn take_optional_epoch(
    fields: &mut ClosedCanonicalObject,
    field: &'static str,
) -> Result<Option<Epoch>, ContractError> {
    fields
        .take_optional(field)?
        .map(|value| super::codec::epoch(value, field))
        .transpose()
}
fn take_profile(
    fields: &mut ClosedCanonicalObject,
    field: &'static str,
) -> Result<ProjectionProfileSetId, ContractError> {
    ProjectionProfileSetId::new(decode_text_field(fields, field)?)
}

fn take_optional_symbol(
    fields: &mut ClosedCanonicalObject,
    field: &'static str,
) -> Result<Option<BoundedSymbolKey>, ContractError> {
    take_optional_text(fields, field)?
        .map(BoundedSymbolKey::new)
        .transpose()
}

/// Decodes one closed canonical object; unknown fields fail closed.
fn decode(value: CanonicalValue) -> Result<PointPayload, ContractError> {
    let mut fields = ClosedCanonicalObject::from_value(value, "point_payload")?;
    let payload = PointPayload::new(
        take_uuid(&mut fields, "installation_incarnation_id")?,
        take_uuid(&mut fields, "collection_generation_id")?,
        take_uuid(&mut fields, "projection_membership_id")?,
        take_uuid(&mut fields, "access_partition_id")?,
        take_uuid(&mut fields, "scoring_partition_id")?,
        take_uuid(&mut fields, "source_id")?,
        take_uuid(&mut fields, "source_revision_id")?,
        take_uuid(&mut fields, "representation_id")?,
        take_uuid(&mut fields, "unit_id")?,
        take_digest(&mut fields, "point_identity_digest_256")?,
        take_uuid(&mut fields, "scoring_document_id")?,
        take_profile(&mut fields, "projection_profile_set_id")?,
        take_enum(&mut fields, "unit_kind", UnitKind::parse)?,
        take_enum(&mut fields, "modality", Modality::parse)?,
        take_name(&mut fields, "language_or_format")?,
        take_optional_enum(&mut fields, "entity_kind", EntityKind::parse)?,
        take_optional_symbol(&mut fields, "normalized_symbol_key")?,
        take_optional_uuid(&mut fields, "repository_lineage_id")?,
        take_epoch(&mut fields, "valid_from_epoch")?,
        take_optional_epoch(&mut fields, "valid_until_epoch_exclusive")?,
    );
    fields.finish()?;
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload() -> PointPayload {
        let digest = Blake3Digest32::from_bytes([0x5a; 32]);
        PointPayload::new(
            InstallationIncarnationId::from_bytes([0x11; 16]),
            CollectionGenerationId::from_bytes([0x12; 16]),
            ProjectionMembershipId::from_bytes([0x13; 16]),
            AccessPartitionId::from_bytes([0x14; 16]),
            ScoringPartitionId::from_bytes([0x15; 16]),
            SourceId::from_bytes([0x16; 16]),
            SourceRevisionId::from_bytes([0x17; 16]),
            RepresentationId::from_bytes([0x18; 16]),
            UnitId::from_bytes([0x19; 16]),
            digest,
            ScoringDocumentId::from_bytes([0x1a; 16]),
            ProjectionProfileSetId::new("lexical-sparse-v1").expect("profile"),
            UnitKind::Symbol,
            Modality::Code,
            BoundedName::new("rust").expect("language"),
            Some(EntityKind::Function),
            Some(BoundedSymbolKey::new("run_pipeline").expect("symbol")),
            Some(RepositoryLineageId::from_bytes([0x1b; 16])),
            Epoch::new(7).expect("from"),
            Some(Epoch::new(9).expect("until")),
        )
    }

    #[test]
    fn canonical_round_trip_is_byte_identical() {
        let value = payload();
        let bytes = value.to_canonical_cbor().expect("encode");
        let restored = PointPayload::from_canonical_cbor(bytes.as_slice()).expect("decode");
        assert_eq!(restored, value);
        assert_eq!(restored.to_canonical_cbor().expect("re-encode"), bytes);
    }

    #[test]
    fn optional_fields_are_absent_not_null() {
        let mut value = payload();
        value.entity_kind = None;
        value.normalized_symbol_key = None;
        value.repository_lineage_id = None;
        value.valid_until_epoch_exclusive = None;
        let object = value.to_canonical_value().expect("value");
        let CanonicalValue::Object(fields) = &object else {
            panic!("payload is an object");
        };
        assert_eq!(fields.len(), 16);
        for name in [
            "entity_kind",
            "normalized_symbol_key",
            "repository_lineage_id",
            "valid_until_epoch_exclusive",
        ] {
            assert!(
                fields.get(&canonical_key(name)).is_none(),
                "{name} must be absent"
            );
        }
    }

    fn canonical_key(name: &str) -> crate::CanonicalKey {
        crate::CanonicalKey::new_non_empty(name).expect("key")
    }

    #[test]
    fn zero_valid_from_is_refused() {
        let mut value = payload();
        value.valid_from_epoch = Epoch::new(0).expect("zero");
        assert_eq!(
            value.to_canonical_cbor().expect_err("zero").kind(),
            ContractErrorKind::InvalidRange
        );
        assert_eq!(
            value.validate().expect_err("zero").field(),
            "valid_from_epoch"
        );
    }

    #[test]
    fn interval_must_be_half_open_and_ascending() {
        let mut value = payload();
        value.valid_from_epoch = Epoch::new(7).expect("from");
        value.valid_until_epoch_exclusive = Some(Epoch::new(7).expect("same"));
        assert_eq!(
            value.validate().expect_err("equal").field(),
            "valid_until_epoch_exclusive"
        );
        value.valid_until_epoch_exclusive = Some(Epoch::new(6).expect("lower"));
        assert!(value.validate().is_err());
        value.valid_until_epoch_exclusive = Some(Epoch::new(8).expect("higher"));
        value.validate().expect("ascending");
    }

    #[test]
    fn maximum_from_with_open_upper_is_legal() {
        let mut value = payload();
        value.valid_from_epoch = Epoch::new(crate::MAX_QDRANT_EPOCH).expect("maximum");
        value.valid_until_epoch_exclusive = None;
        value.validate().expect("open interval at the ceiling");
        // A closed upper bound strictly greater than the maximum is unrepresentable,
        // so the pair is refused by construction rather than by a special case.
        assert!(value.to_canonical_cbor().is_ok());
    }

    #[test]
    fn unknown_and_wrong_typed_fields_fail_closed() {
        let value = payload();
        let object = value.to_canonical_value().expect("value");
        let CanonicalValue::Object(fields) = object else {
            panic!("payload is an object");
        };
        let mut unknown = fields.clone();
        unknown
            .insert(
                canonical_key("source_text"),
                CanonicalValue::Text(crate::CanonicalText::new("x").expect("text")),
            )
            .expect("bounded extra field");
        assert_eq!(
            PointPayload::from_canonical_value(CanonicalValue::Object(unknown))
                .expect_err("unknown field")
                .kind(),
            ContractErrorKind::UnknownField
        );

        let mut wrong = fields;
        wrong.remove(&canonical_key("valid_from_epoch"));
        wrong
            .insert(
                canonical_key("valid_from_epoch"),
                CanonicalValue::Bool(true),
            )
            .expect("replace field");
        assert_eq!(
            PointPayload::from_canonical_value(CanonicalValue::Object(wrong))
                .expect_err("wrong type")
                .kind(),
            ContractErrorKind::MalformedPayload
        );
    }

    #[test]
    fn missing_required_field_fails_closed() {
        let value = payload();
        let CanonicalValue::Object(mut fields) = value.to_canonical_value().expect("value") else {
            panic!("payload is an object");
        };
        fields.remove(&canonical_key("source_id"));
        assert_eq!(
            PointPayload::from_canonical_value(CanonicalValue::Object(fields))
                .expect_err("missing required")
                .field(),
            "source_id"
        );
    }

    #[test]
    fn identity_is_internal_and_stable() {
        let value = payload();
        let first = value.identity().expect("identity");
        assert_eq!(first, value.identity().expect("identity again"));

        let mut changed = payload();
        changed.unit_kind = UnitKind::Doc;
        assert_ne!(first, changed.identity().expect("changed identity"));
    }
}
