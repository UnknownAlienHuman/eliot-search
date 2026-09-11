const fn strong_ordering() -> WriteOrdering {
    WriteOrdering {
        r#type: WriteOrderingType::Strong as i32,
    }
}

fn update_completed(status: i32) -> bool {
    UpdateStatus::try_from(status).is_ok_and(|parsed| parsed == UpdateStatus::Completed)
}

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

fn get_string(payload: &HashMap<String, Value>, key: &str) -> Result<String, BridgeError> {
    match payload.get(key).and_then(|entry| entry.kind.as_ref()) {
        Some(value::Kind::StringValue(text)) => Ok(text.clone()),
        _ => Err(BridgeError::MalformedResponse),
    }
}

fn get_int(payload: &HashMap<String, Value>, key: &str) -> Result<i64, BridgeError> {
    match payload.get(key).and_then(|entry| entry.kind.as_ref()) {
        Some(value::Kind::IntegerValue(number)) => Ok(*number),
        _ => Err(BridgeError::MalformedResponse),
    }
}

fn encode_payload(point: &PointRecord) -> Result<HashMap<String, Value>, BridgeError> {
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
            i64::try_from(payload.source_revision).map_err(|_| BridgeError::MutationTooLarge)?,
        ),
    );
    map.insert(
        "unit_ordinal".to_owned(),
        int_value(i64::try_from(payload.unit_ordinal).map_err(|_| BridgeError::MutationTooLarge)?),
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

fn decode_payload(payload: &HashMap<String, Value>) -> Result<PointPayload, BridgeError> {
    let access = hex_to_32(&get_string(payload, EligibilityFilter::INDEXED_FIELDS[0])?)?;
    let source_membership =
        OpaqueId::new(get_string(payload, EligibilityFilter::INDEXED_FIELDS[1])?)
            .map_err(|_| BridgeError::MalformedResponse)?;
    let projection_membership = OpaqueId::new(get_string(payload, "projection_membership_id")?)
        .map_err(|_| BridgeError::MalformedResponse)?;
    let source_revision = u64::try_from(get_int(payload, "source_revision")?)
        .map_err(|_| BridgeError::MalformedResponse)?;
    let unit_ordinal = u64::try_from(get_int(payload, "unit_ordinal")?)
        .map_err(|_| BridgeError::MalformedResponse)?;
    let valid_from =
        search_contracts::Epoch::new(get_int(payload, EligibilityFilter::INDEXED_FIELDS[2])?)
            .map_err(|_| BridgeError::MalformedResponse)?;
    let valid_until = if payload.contains_key(EligibilityFilter::INDEXED_FIELDS[3]) {
        Some(
            search_contracts::Epoch::new(get_int(payload, EligibilityFilter::INDEXED_FIELDS[3])?)
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
        access_partition_digest: search_contracts::Blake3Digest32::from_bytes(access),
        source_revision,
        unit_ordinal,
        valid_from_epoch: valid_from,
        valid_until_epoch_exclusive: valid_until,
        payload_digest: search_contracts::Blake3Digest32::from_bytes(payload_digest),
        identity_digest: search_contracts::Blake3Digest32::from_bytes(identity_digest),
    })
}

fn encode_vectors(point: &PointRecord) -> Vectors {
    let mut map = HashMap::new();
    for (name, stored) in &point.vectors {
        let (indices, values): (Vec<u32>, Vec<f32>) = stored.values.iter().copied().unzip();
        map.insert(name.clone(), Vector::new_sparse(indices, values));
    }
    Vectors {
        vectors_options: Some(vectors::VectorsOptions::Vectors(
            qdrant_client::qdrant::NamedVectors { vectors: map },
        )),
    }
}

fn decode_vectors(
    output: Option<&qdrant_client::qdrant::VectorsOutput>,
    payload: &HashMap<String, Value>,
    schema: &CollectionSchema,
) -> Result<BTreeMap<String, StoredVector>, BridgeError> {
    let named = match output.and_then(|vectors| vectors.vectors_options.as_ref()) {
        Some(vectors_output::VectorsOptions::Vectors(named)) => &named.vectors,
        _ => return Err(BridgeError::MalformedResponse),
    };
    if named.len() != schema.named_vectors.len() {
        return Err(BridgeError::MalformedResponse);
    }
    let mut decoded = BTreeMap::new();
    for (name, vector_schema) in &schema.named_vectors {
        let entry = named.get(name).ok_or(BridgeError::MalformedResponse)?;
        let Some(vector_output::Vector::Sparse(sparse)) = entry.vector.as_ref() else {
            return Err(BridgeError::MalformedResponse);
        };
        if sparse.indices.len() != sparse.values.len() {
            return Err(BridgeError::MalformedResponse);
        }
        let mut values = Vec::with_capacity(sparse.indices.len());
        for (index, score) in sparse.indices.iter().zip(sparse.values.iter()) {
            if !score.is_finite() {
                return Err(BridgeError::MalformedResponse);
            }
            values.push((*index, *score));
        }
        if values.is_empty()
            || values.windows(2).any(|pair| pair[0].0 >= pair[1].0)
            || values
                .last()
                .is_some_and(|(index, _)| *index >= vector_schema.dimensions)
        {
            return Err(BridgeError::MalformedResponse);
        }
        let digest = hex_to_32(&get_string(payload, &format!("vector_digest_{name}"))?)?;
        decoded.insert(
            name.clone(),
            StoredVector {
                dimensions: vector_schema.dimensions,
                sparse: vector_schema.sparse,
                values,
                digest: search_contracts::Blake3Digest32::from_bytes(digest),
            },
        );
    }
    Ok(decoded)
}

fn decode_point(
    id: &PointId,
    payload: &HashMap<String, Value>,
    vectors: Option<&qdrant_client::qdrant::VectorsOutput>,
    schema: &CollectionSchema,
) -> Result<PointRecord, BridgeError> {
    Ok(PointRecord {
        point_id: bridge_point_id(id)?,
        payload: decode_payload(payload)?,
        vectors: decode_vectors(vectors, payload, schema)?,
    })
}
