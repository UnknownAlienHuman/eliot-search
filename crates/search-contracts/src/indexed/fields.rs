use crate::{CanonicalValue, ContractError, ContractErrorKind};

use super::codec::{array, error, object, text};

/// Closed provider-neutral payload-index kinds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayloadIndexKind {
    Uuid,
    Keyword,
    Integer,
}

impl PayloadIndexKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Uuid => "uuid",
            Self::Keyword => "keyword",
            Self::Integer => "integer",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ContractError> {
        match value {
            "uuid" => Ok(Self::Uuid),
            "keyword" => Ok(Self::Keyword),
            "integer" => Ok(Self::Integer),
            _ => Err(error(
                ContractErrorKind::InvalidTaggedVariant,
                "payload_index_kind",
            )),
        }
    }
}

/// Field identity is closed; provider strings are obtained from its descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayloadField {
    InstallationIncarnationId,
    CollectionGenerationId,
    ProjectionMembershipId,
    AccessPartitionId,
    ScoringPartitionId,
    SourceId,
    SourceRevisionId,
    RepresentationId,
    UnitId,
    PointIdentityDigest256,
    ScoringDocumentId,
    ProjectionProfileSetId,
    UnitKind,
    Modality,
    LanguageOrFormat,
    EntityKind,
    NormalizedSymbolKey,
    RepositoryLineageId,
    ValidFromEpoch,
    ValidUntilEpochExclusive,
}

impl PayloadField {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        self.descriptor().name
    }

    #[must_use]
    pub fn descriptor(self) -> &'static PayloadFieldDescriptor {
        // Every variant is represented exactly once by the single ordered table.
        &PAYLOAD_FIELDS[self as usize]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayloadFieldDescriptor {
    pub field: PayloadField,
    pub name: &'static str,
    pub kind: PayloadIndexKind,
    pub optional: bool,
    pub indexed: bool,
}

const fn field(
    field: PayloadField,
    name: &'static str,
    kind: PayloadIndexKind,
    optional: bool,
    indexed: bool,
) -> PayloadFieldDescriptor {
    PayloadFieldDescriptor {
        field,
        name,
        kind,
        optional,
        indexed,
    }
}

/// The sole ordered S9.5 table. Index plans are derived, never copied.
pub const PAYLOAD_FIELDS: [PayloadFieldDescriptor; 20] = [
    field(
        PayloadField::InstallationIncarnationId,
        "installation_incarnation_id",
        PayloadIndexKind::Uuid,
        false,
        true,
    ),
    field(
        PayloadField::CollectionGenerationId,
        "collection_generation_id",
        PayloadIndexKind::Uuid,
        false,
        true,
    ),
    field(
        PayloadField::ProjectionMembershipId,
        "projection_membership_id",
        PayloadIndexKind::Uuid,
        false,
        true,
    ),
    field(
        PayloadField::AccessPartitionId,
        "access_partition_id",
        PayloadIndexKind::Uuid,
        false,
        true,
    ),
    field(
        PayloadField::ScoringPartitionId,
        "scoring_partition_id",
        PayloadIndexKind::Uuid,
        false,
        true,
    ),
    field(
        PayloadField::SourceId,
        "source_id",
        PayloadIndexKind::Uuid,
        false,
        true,
    ),
    field(
        PayloadField::SourceRevisionId,
        "source_revision_id",
        PayloadIndexKind::Uuid,
        false,
        true,
    ),
    field(
        PayloadField::RepresentationId,
        "representation_id",
        PayloadIndexKind::Uuid,
        false,
        true,
    ),
    field(
        PayloadField::UnitId,
        "unit_id",
        PayloadIndexKind::Uuid,
        false,
        true,
    ),
    field(
        PayloadField::PointIdentityDigest256,
        "point_identity_digest_256",
        PayloadIndexKind::Keyword,
        false,
        false,
    ),
    field(
        PayloadField::ScoringDocumentId,
        "scoring_document_id",
        PayloadIndexKind::Uuid,
        false,
        true,
    ),
    field(
        PayloadField::ProjectionProfileSetId,
        "projection_profile_set_id",
        PayloadIndexKind::Keyword,
        false,
        true,
    ),
    field(
        PayloadField::UnitKind,
        "unit_kind",
        PayloadIndexKind::Keyword,
        false,
        true,
    ),
    field(
        PayloadField::Modality,
        "modality",
        PayloadIndexKind::Keyword,
        false,
        true,
    ),
    field(
        PayloadField::LanguageOrFormat,
        "language_or_format",
        PayloadIndexKind::Keyword,
        false,
        true,
    ),
    field(
        PayloadField::EntityKind,
        "entity_kind",
        PayloadIndexKind::Keyword,
        true,
        true,
    ),
    field(
        PayloadField::NormalizedSymbolKey,
        "normalized_symbol_key",
        PayloadIndexKind::Keyword,
        true,
        true,
    ),
    field(
        PayloadField::RepositoryLineageId,
        "repository_lineage_id",
        PayloadIndexKind::Uuid,
        true,
        true,
    ),
    field(
        PayloadField::ValidFromEpoch,
        "valid_from_epoch",
        PayloadIndexKind::Integer,
        false,
        true,
    ),
    field(
        PayloadField::ValidUntilEpochExclusive,
        "valid_until_epoch_exclusive",
        PayloadIndexKind::Integer,
        true,
        true,
    ),
];

pub const PAYLOAD_INDEX_COUNT: usize = 19;

/// Exact baseline index table in payload declaration order.
pub fn payload_indexes() -> impl Iterator<Item = &'static PayloadFieldDescriptor> {
    PAYLOAD_FIELDS.iter().filter(|field| field.indexed)
}

/// Validates readback independent of provider map order, preserving closed kinds.
pub fn validate_payload_indexes(
    indexes: &[(String, PayloadIndexKind)],
) -> Result<(), ContractError> {
    if indexes.len() > PAYLOAD_INDEX_COUNT {
        return Err(error(
            ContractErrorKind::UnknownField,
            "payload_index_extra",
        ));
    }
    for (position, (name, kind)) in indexes.iter().enumerate() {
        let expected = payload_indexes()
            .find(|field| field.name == name)
            .ok_or_else(|| error(ContractErrorKind::UnknownField, "payload_index_extra"))?;
        if indexes[..position].iter().any(|(other, _)| other == name) {
            return Err(error(ContractErrorKind::Duplicate, "payload_index"));
        }
        if *kind != expected.kind {
            return Err(error(
                ContractErrorKind::MalformedPayload,
                "payload_index_type",
            ));
        }
    }
    if indexes.len() != PAYLOAD_INDEX_COUNT {
        return Err(error(
            ContractErrorKind::MalformedPayload,
            "payload_index_missing",
        ));
    }
    Ok(())
}

/// Schema preimage component, not an independently supplied schema identity.
pub fn payload_plan_value() -> Result<CanonicalValue, ContractError> {
    array(
        PAYLOAD_FIELDS
            .iter()
            .map(|field| {
                object([
                    ("name", text(field.name)?),
                    ("kind", text(field.kind.as_str())?),
                    ("optional", CanonicalValue::Bool(field.optional)),
                    ("indexed", CanonicalValue::Bool(field.indexed)),
                ])
            })
            .collect::<Result<Vec<_>, ContractError>>()?,
    )
}

pub fn index_plan_value() -> Result<CanonicalValue, ContractError> {
    array(
        payload_indexes()
            .map(|field| {
                object([
                    ("name", text(field.name)?),
                    ("kind", text(field.kind.as_str())?),
                ])
            })
            .collect::<Result<Vec<_>, ContractError>>()?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn indexes() -> Vec<(String, PayloadIndexKind)> {
        payload_indexes()
            .map(|field| (field.name.to_owned(), field.kind))
            .collect()
    }

    #[test]
    fn exact_order_and_index_kinds_are_frozen() {
        use PayloadIndexKind::{Integer, Keyword, Uuid};
        // Independent normative tuples detect per-field kind/absence drift even
        // when names and aggregate kind counts remain unchanged.
        let expected = [
            ("installation_incarnation_id", Uuid, false, true),
            ("collection_generation_id", Uuid, false, true),
            ("projection_membership_id", Uuid, false, true),
            ("access_partition_id", Uuid, false, true),
            ("scoring_partition_id", Uuid, false, true),
            ("source_id", Uuid, false, true),
            ("source_revision_id", Uuid, false, true),
            ("representation_id", Uuid, false, true),
            ("unit_id", Uuid, false, true),
            ("point_identity_digest_256", Keyword, false, false),
            ("scoring_document_id", Uuid, false, true),
            ("projection_profile_set_id", Keyword, false, true),
            ("unit_kind", Keyword, false, true),
            ("modality", Keyword, false, true),
            ("language_or_format", Keyword, false, true),
            ("entity_kind", Keyword, true, true),
            ("normalized_symbol_key", Keyword, true, true),
            ("repository_lineage_id", Uuid, true, true),
            ("valid_from_epoch", Integer, false, true),
            ("valid_until_epoch_exclusive", Integer, true, true),
        ];
        assert_eq!(
            PAYLOAD_FIELDS.map(|field| (field.name, field.kind, field.optional, field.indexed)),
            expected,
        );
        for field in PAYLOAD_FIELDS {
            assert_eq!(field.field.descriptor(), &field);
            assert_eq!(field.field.as_str(), field.name);
        }
        let indexes = indexes();
        assert_eq!(indexes.len(), 19);
        assert_eq!(
            indexes
                .iter()
                .filter(|(_, kind)| *kind == PayloadIndexKind::Uuid)
                .count(),
            11
        );
        assert_eq!(
            indexes
                .iter()
                .filter(|(_, kind)| *kind == PayloadIndexKind::Keyword)
                .count(),
            6
        );
        assert_eq!(
            indexes
                .iter()
                .filter(|(_, kind)| *kind == PayloadIndexKind::Integer)
                .count(),
            2
        );
        assert!(
            !indexes
                .iter()
                .any(|(name, _)| name == "point_identity_digest_256")
        );
    }

    #[test]
    fn index_readback_rejects_missing_extra_duplicate_and_wrong_type() {
        let expected = indexes();
        validate_payload_indexes(&expected).expect("exact indexes");
        let mut permuted = expected.clone();
        permuted.reverse();
        validate_payload_indexes(&permuted).expect("provider map order is irrelevant");
        let mut missing = expected.clone();
        missing.pop();
        assert_eq!(
            validate_payload_indexes(&missing)
                .expect_err("missing")
                .field(),
            "payload_index_missing"
        );
        let mut extra = expected.clone();
        extra.push(("source_text".to_owned(), PayloadIndexKind::Keyword));
        assert_eq!(
            validate_payload_indexes(&extra).expect_err("extra").field(),
            "payload_index_extra"
        );
        let mut wrong = expected.clone();
        wrong[0].1 = PayloadIndexKind::Keyword;
        assert_eq!(
            validate_payload_indexes(&wrong)
                .expect_err("wrong type")
                .field(),
            "payload_index_type"
        );
        let mut duplicate = expected;
        duplicate[1] = duplicate[0].clone();
        assert_eq!(
            validate_payload_indexes(&duplicate)
                .expect_err("duplicate")
                .kind(),
            ContractErrorKind::Duplicate
        );
    }
}
