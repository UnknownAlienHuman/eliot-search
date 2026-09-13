fn str_value(text: &str) -> Value {
    Value {
        kind: Some(value::Kind::StringValue(text.to_owned())),
    }
}

const fn int_value(number: i64) -> Value {
    Value {
        kind: Some(value::Kind::IntegerValue(number)),
    }
}

fn get_string(
    payload: &HashMap<String, Value>,
    key: &str,
) -> Result<String, BridgeError> {
    match payload.get(key).and_then(|entry| entry.kind.as_ref()) {
        Some(value::Kind::StringValue(text)) => Ok(text.clone()),
        _ => Err(BridgeError::MalformedResponse),
    }
}

fn get_int(
    payload: &HashMap<String, Value>,
    key: &str,
) -> Result<i64, BridgeError> {
    match payload.get(key).and_then(|entry| entry.kind.as_ref()) {
        Some(value::Kind::IntegerValue(number)) => Ok(*number),
        _ => Err(BridgeError::MalformedResponse),
    }
}

fn encode_payload(
    point: &PointRecord,
) -> Result<HashMap<String, Value>, BridgeError> {
    let payload = &point.payload;
    let mut map = HashMap::new();
    map.insert(
        EligibilityFilter::INDEXED_FIELDS[0].to_owned(),
        str_value(&hex_from_32(payload.access_partition_digest.as_bytes())),
    );
    map.insert(
        EligibilityFilter::INDEXED_FIELDS[1].to_owned(),
        str_value(payload.source_membership_id.as_str()),
    );
    map.insert(
        "projection_membership_id".to_owned(),
        str_value(payload.projection_membership_id.as_str()),
    );
    map.insert(
        "source_revision".to_owned(),
        int_value(
            i64::try_from(payload.source_revision)
                .map_err(|_| BridgeError::MutationTooLarge)?,
        ),
    );
    map.insert(
        "unit_ordinal".to_owned(),
        int_value(
            i64::try_from(payload.unit_ordinal)
                .map_err(|_| BridgeError::MutationTooLarge)?,
        ),
    );
    map.insert(
        EligibilityFilter::INDEXED_FIELDS[2].to_owned(),
        int_value(payload.valid_from_epoch.get()),
    );
    if let Some(until) = payload.valid_until_epoch_exclusive {
        map.insert(
            EligibilityFilter::INDEXED_FIELDS[3].to_owned(),
            int_value(until.get()),
        );
    }
    map.insert(
        "payload_digest".to_owned(),
        str_value(&hex_from_32(payload.payload_digest.as_bytes())),
    );
    map.insert(
        "identity_digest".to_owned(),
        str_value(&hex_from_32(payload.identity_digest.as_bytes())),
    );
    for (name, vector) in &point.vectors {
        map.insert(
            format!("vector_digest_{name}"),
            str_value(&hex_from_32(vector.digest.as_bytes())),
        );
    }
    Ok(map)
}

fn decode_payload(
    payload: &HashMap<String, Value>,
) -> Result<PointPayload, BridgeError> {
    let access =
        hex_to_32(&get_string(payload, EligibilityFilter::INDEXED_FIELDS[0])?)?;
    let source_membership =
        OpaqueId::new(get_string(payload, EligibilityFilter::INDEXED_FIELDS[1])?)
            .map_err(|_| BridgeError::MalformedResponse)?;
    let projection_membership =
        OpaqueId::new(get_string(payload, "projection_membership_id")?)
            .map_err(|_| BridgeError::MalformedResponse)?;
    let source_revision = u64::try_from(get_int(payload, "source_revision")?)
        .map_err(|_| BridgeError::MalformedResponse)?;
    let unit_ordinal = u64::try_from(get_int(payload, "unit_ordinal")?)
        .map_err(|_| BridgeError::MalformedResponse)?;
    let valid_from = search_contracts::Epoch::new(get_int(
        payload,
        EligibilityFilter::INDEXED_FIELDS[2],
    )?)
    .map_err(|_| BridgeError::MalformedResponse)?;
    let valid_until = if payload.contains_key(EligibilityFilter::INDEXED_FIELDS[3]) {
        Some(
            search_contracts::Epoch::new(get_int(
                payload,
                EligibilityFilter::INDEXED_FIELDS[3],
            )?)
            .map_err(|_| BridgeError::MalformedResponse)?,
        )
    } else {
        None
    };
    let payload_digest = hex_to_32(&get_string(payload, "payload_digest")?)?;
    let identity_digest = hex_to_32(&get_string(payload, "identity_digest")?)?;
    Ok(PointPayload {
        source_membership_id: source_membership,
        projection_membership_id: projection_membership,
        access_partition_digest: search_contracts::Blake3Digest32::from_bytes(
            access,
        ),
        source_revision,
        unit_ordinal,
        valid_from_epoch: valid_from,
        valid_until_epoch_exclusive: valid_until,
        payload_digest: search_contracts::Blake3Digest32::from_bytes(
            payload_digest,
        ),
        identity_digest: search_contracts::Blake3Digest32::from_bytes(
            identity_digest,
        ),
    })
}
