impl RealDataPlane {
    /// Reads back exactly the requested identifiers with explicit
    /// missing/unexpected sets.
    ///
    /// Qdrant response order is not part of the bridge contract. Returned
    /// points and missing IDs therefore follow the sorted validated request;
    /// unexpected IDs are sorted independently. Duplicate response IDs fail
    /// closed as a malformed response.
    pub async fn readback_exact(
        &self,
        route: &CollectionRoute,
        ids: Vec<QdrantPointId>,
        context: &OpContext,
    ) -> Result<BoundedPointReadback, BridgeError> {
        context.check()?;
        let ids = validate_exact_ids(ids, self.limits.max_points_per_mutation)?;
        let name = collection_name(route)?;
        let schema = self
            .schemas
            .get(&name)
            .ok_or(BridgeError::CollectionNotFound)?;
        let readback = tokio::time::timeout(
            context.deadline(),
            self.client.get_points(GetPoints {
                collection_name: name.clone(),
                ids: ids.iter().map(vendor_point_id).collect(),
                with_payload: Some(true.into()),
                with_vectors: Some(true.into()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::TransportFailed)?
        .map_err(map_read_error)?;
        if readback.result.len() > ids.len() {
            return Err(BridgeError::MalformedResponse);
        }

        let requested: BTreeSet<QdrantPointId> = ids.iter().copied().collect();
        let mut returned = BTreeMap::<QdrantPointId, PointRecord>::new();
        let mut unexpected = BTreeSet::<QdrantPointId>::new();
        for retrieved in &readback.result {
            let vendor_id = retrieved
                .id
                .as_ref()
                .ok_or(BridgeError::MalformedResponse)?;
            let id = bridge_point_id(vendor_id)?;
            if !requested.contains(&id) {
                if !unexpected.insert(id) {
                    return Err(BridgeError::MalformedResponse);
                }
                continue;
            }
            let point = decode_point(
                vendor_id,
                &retrieved.payload,
                retrieved.vectors.as_ref(),
                schema,
            )?;
            if returned.insert(id, point).is_some() {
                return Err(BridgeError::MalformedResponse);
            }
        }

        let mut points = Vec::with_capacity(returned.len());
        let mut missing_ids = Vec::new();
        for id in ids {
            match returned.remove(&id) {
                Some(point) => points.push(point),
                None => missing_ids.push(id),
            }
        }
        Ok(BoundedPointReadback {
            points,
            missing_ids,
            unexpected_ids: unexpected.into_iter().collect(),
        })
    }
}
