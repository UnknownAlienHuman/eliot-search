pub(super) fn encode_vectors(point: &PointRecord) -> Vectors {
    let mut map = HashMap::new();
    for (name, stored) in &point.vectors {
        let (indices, values): (Vec<u32>, Vec<f32>) =
            stored.values.iter().copied().unzip();
        map.insert(name.clone(), Vector::new_sparse(indices, values));
    }
    Vectors {
        vectors_options: Some(vectors::VectorsOptions::Vectors(
            qdrant_client::qdrant::NamedVectors { vectors: map },
        )),
    }
}

pub(super) fn decode_vectors(
    output: Option<&qdrant_client::qdrant::VectorsOutput>,
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
        let Some(vector_output::Vector::Sparse(sparse)) = entry.vector.as_ref()
        else {
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
        let stored = StoredVector {
            dimensions: vector_schema.dimensions,
            sparse: vector_schema.sparse,
            values,
        };
        stored
            .validate(*vector_schema)
            .map_err(|_| BridgeError::MalformedResponse)?;
        decoded.insert(name.clone(), stored);
    }
    Ok(decoded)
}
