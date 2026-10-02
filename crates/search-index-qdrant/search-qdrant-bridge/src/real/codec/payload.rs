use search_contracts::{
    AccessPartitionId, Blake3Digest32, BoundedSymbolKey, CollectionGenerationId, EntityKind,
    Epoch, InstallationIncarnationId, Modality, ProfileId, ProjectionMembershipId,
    ProjectionProfileSetId, RepositoryLineageId, RepresentationId, ScoringDocumentId,
    ScoringPartitionId, SourceId, SourceRevisionId, UnitId, UnitKind,
};

pub(super) fn str_value(text: &str) -> Value {
    Value {
        kind: Some(value::Kind::StringValue(text.to_owned())),
    }
}

pub(super) const fn int_value(number: i64) -> Value {
    Value {
        kind: Some(value::Kind::IntegerValue(number)),
    }
}

pub(super) fn get_string(payload: &HashMap<String, Value>, key: &str) -> Result<String, BridgeError> {
    match payload.get(key).and_then(|entry| entry.kind.as_ref()) {
        Some(value::Kind::StringValue(text)) => Ok(text.clone()),
        _ => Err(BridgeError::MalformedResponse),
    }
}

pub(super) fn get_optional_string(
    payload: &HashMap<String, Value>,
    key: &str,
) -> Result<Option<String>, BridgeError> {
    payload.get(key).map_or(Ok(None), |entry| {
        match entry.kind.as_ref() {
            Some(value::Kind::StringValue(text)) => Ok(Some(text.clone())),
            _ => Err(BridgeError::MalformedResponse),
        }
    })
}

pub(super) fn get_int(payload: &HashMap<String, Value>, key: &str) -> Result<i64, BridgeError> {
    match payload.get(key).and_then(|entry| entry.kind.as_ref()) {
        Some(value::Kind::IntegerValue(number)) => Ok(*number),
        _ => Err(BridgeError::MalformedResponse),
    }
}

pub(super) fn ensure_closed_payload(payload: &HashMap<String, Value>) -> Result<(), BridgeError> {
    if payload
        .keys()
        .any(|key| !PointPayload::PAYLOAD_FIELDS.contains(&key.as_str()))
    {
        return Err(BridgeError::MalformedResponse);
    }
    Ok(())
}

pub(super) fn encode_payload(point: &PointRecord) -> Result<HashMap<String, Value>, BridgeError> {
    let payload = &point.payload;
    payload.validate()?;
    let mut map = HashMap::new();
    map.insert(
        PointPayload::INSTALLATION_INCARNATION_FIELD.to_owned(),
        str_value(&payload.installation_incarnation_id.to_string()),
    );
    map.insert(
        PointPayload::COLLECTION_GENERATION_FIELD.to_owned(),
        str_value(&payload.collection_generation_id.to_string()),
    );
    map.insert(
        PointPayload::PROJECTION_MEMBERSHIP_FIELD.to_owned(),
        str_value(&payload.projection_membership_id.to_string()),
    );
    map.insert(
        PointPayload::ACCESS_PARTITION_FIELD.to_owned(),
        str_value(&payload.access_partition_id.to_string()),
    );
    map.insert(
        PointPayload::SCORING_PARTITION_FIELD.to_owned(),
        str_value(&payload.scoring_partition_id.to_string()),
    );
    map.insert(
        PointPayload::SOURCE_ID_FIELD.to_owned(),
        str_value(&payload.source_id.to_string()),
    );
    map.insert(
        PointPayload::SOURCE_REVISION_FIELD.to_owned(),
        str_value(&payload.source_revision_id.to_string()),
    );
    map.insert(
        PointPayload::REPRESENTATION_FIELD.to_owned(),
        str_value(&payload.representation_id.to_string()),
    );
    map.insert(
        PointPayload::UNIT_FIELD.to_owned(),
        str_value(&payload.unit_id.to_string()),
    );
    map.insert(
        PointPayload::POINT_IDENTITY_DIGEST_FIELD.to_owned(),
        str_value(&hex_from_32(payload.point_identity_digest_256.as_bytes())),
    );
    map.insert(
        PointPayload::SCORING_DOCUMENT_FIELD.to_owned(),
        str_value(&payload.scoring_document_id.to_string()),
    );
    map.insert(
        PointPayload::PROJECTION_PROFILE_SET_FIELD.to_owned(),
        str_value(payload.projection_profile_set_id.as_str()),
    );
    map.insert(
        PointPayload::UNIT_KIND_FIELD.to_owned(),
        str_value(payload.unit_kind.as_str()),
    );
    map.insert(
        PointPayload::MODALITY_FIELD.to_owned(),
        str_value(payload.modality.as_str()),
    );
    map.insert(
        PointPayload::LANGUAGE_OR_FORMAT_FIELD.to_owned(),
        str_value(payload.language_or_format.as_str()),
    );
    if let Some(entity_kind) = payload.entity_kind {
        map.insert(
            PointPayload::ENTITY_KIND_FIELD.to_owned(),
            str_value(entity_kind.as_str()),
        );
    }
    if let Some(symbol) = &payload.normalized_symbol_key {
        map.insert(
            PointPayload::NORMALIZED_SYMBOL_FIELD.to_owned(),
            str_value(symbol.as_str()),
        );
    }
    if let Some(lineage) = payload.repository_lineage_id {
        map.insert(
            PointPayload::REPOSITORY_LINEAGE_FIELD.to_owned(),
            str_value(&lineage.to_string()),
        );
    }
    map.insert(
        PointPayload::VALID_FROM_FIELD.to_owned(),
        int_value(payload.valid_from_epoch.get()),
    );
    if let Some(until) = payload.valid_until_epoch_exclusive {
        map.insert(
            PointPayload::VALID_UNTIL_FIELD.to_owned(),
            int_value(until.get()),
        );
    }
    Ok(map)
}

pub(super) fn decode_payload(payload: &HashMap<String, Value>) -> Result<PointPayload, BridgeError> {
    ensure_closed_payload(payload)?;
    let decoded = PointPayload {
        installation_incarnation_id: InstallationIncarnationId::parse(&get_string(
            payload,
            PointPayload::INSTALLATION_INCARNATION_FIELD,
        )?)
        .map_err(|_| BridgeError::MalformedResponse)?,
        collection_generation_id: CollectionGenerationId::parse(&get_string(
            payload,
            PointPayload::COLLECTION_GENERATION_FIELD,
        )?)
        .map_err(|_| BridgeError::MalformedResponse)?,
        projection_membership_id: ProjectionMembershipId::parse(&get_string(
            payload,
            PointPayload::PROJECTION_MEMBERSHIP_FIELD,
        )?)
        .map_err(|_| BridgeError::MalformedResponse)?,
        access_partition_id: AccessPartitionId::parse(&get_string(
            payload,
            PointPayload::ACCESS_PARTITION_FIELD,
        )?)
        .map_err(|_| BridgeError::MalformedResponse)?,
        scoring_partition_id: ScoringPartitionId::parse(&get_string(
            payload,
            PointPayload::SCORING_PARTITION_FIELD,
        )?)
        .map_err(|_| BridgeError::MalformedResponse)?,
        source_id: SourceId::parse(&get_string(payload, PointPayload::SOURCE_ID_FIELD)?)
            .map_err(|_| BridgeError::MalformedResponse)?,
        source_revision_id: SourceRevisionId::parse(&get_string(
            payload,
            PointPayload::SOURCE_REVISION_FIELD,
        )?)
        .map_err(|_| BridgeError::MalformedResponse)?,
        representation_id: RepresentationId::parse(&get_string(
            payload,
            PointPayload::REPRESENTATION_FIELD,
        )?)
        .map_err(|_| BridgeError::MalformedResponse)?,
        unit_id: UnitId::parse(&get_string(payload, PointPayload::UNIT_FIELD)?)
            .map_err(|_| BridgeError::MalformedResponse)?,
        point_identity_digest_256: Blake3Digest32::parse_hex(&get_string(
            payload,
            PointPayload::POINT_IDENTITY_DIGEST_FIELD,
        )?)
        .map_err(|_| BridgeError::MalformedResponse)?,
        scoring_document_id: ScoringDocumentId::parse(&get_string(
            payload,
            PointPayload::SCORING_DOCUMENT_FIELD,
        )?)
        .map_err(|_| BridgeError::MalformedResponse)?,
        projection_profile_set_id: ProjectionProfileSetId::new(get_string(
            payload,
            PointPayload::PROJECTION_PROFILE_SET_FIELD,
        )?)
        .map_err(|_| BridgeError::MalformedResponse)?,
        unit_kind: UnitKind::parse(&get_string(payload, PointPayload::UNIT_KIND_FIELD)?)
            .map_err(|_| BridgeError::MalformedResponse)?,
        modality: Modality::parse(&get_string(payload, PointPayload::MODALITY_FIELD)?)
            .map_err(|_| BridgeError::MalformedResponse)?,
        language_or_format: ProfileId::new(get_string(
            payload,
            PointPayload::LANGUAGE_OR_FORMAT_FIELD,
        )?)
        .map_err(|_| BridgeError::MalformedResponse)?,
        entity_kind: get_optional_string(payload, PointPayload::ENTITY_KIND_FIELD)?
            .map(|value| EntityKind::parse(&value))
            .transpose()
            .map_err(|_| BridgeError::MalformedResponse)?,
        normalized_symbol_key: get_optional_string(
            payload,
            PointPayload::NORMALIZED_SYMBOL_FIELD,
        )?
        .map(BoundedSymbolKey::new)
        .transpose()
        .map_err(|_| BridgeError::MalformedResponse)?,
        repository_lineage_id: get_optional_string(
            payload,
            PointPayload::REPOSITORY_LINEAGE_FIELD,
        )?
        .map(|value| RepositoryLineageId::parse(&value))
        .transpose()
        .map_err(|_| BridgeError::MalformedResponse)?,
        valid_from_epoch: Epoch::new(get_int(payload, PointPayload::VALID_FROM_FIELD)?)
            .map_err(|_| BridgeError::MalformedResponse)?,
        valid_until_epoch_exclusive: if payload.contains_key(PointPayload::VALID_UNTIL_FIELD) {
            Some(
                Epoch::new(get_int(payload, PointPayload::VALID_UNTIL_FIELD)?)
                    .map_err(|_| BridgeError::MalformedResponse)?,
            )
        } else {
            None
        },
    };
    decoded
        .validate()
        .map_err(|_| BridgeError::MalformedResponse)?;
    Ok(decoded)
}
