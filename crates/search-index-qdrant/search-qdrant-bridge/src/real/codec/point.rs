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
