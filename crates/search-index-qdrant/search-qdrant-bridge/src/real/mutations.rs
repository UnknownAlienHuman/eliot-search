impl RealDataPlane {
    /// Upserts only explicit point IDs with `wait=true`, strong ordering and
    /// exact readback before success. Same identity plus same canonical batch
    /// replays without a second write; same identity plus different input is
    /// [`BridgeError::OperationConflict`].
    pub async fn upsert_exact(
        &mut self,
        route: &CollectionRoute,
        points: Vec<PointRecord>,
        mutation: BridgeMutation,
        context: &OpContext,
    ) -> Result<MutationReceipt, BridgeError> {
        context.check()?;
        if let Some(replay) = self.replay(&mutation)? {
            return Ok(replay);
        }
        if self.operations.len() >= self.limits.max_operation_receipts {
            return Err(BridgeError::MutationTooLarge);
        }
        if points.is_empty() || points.len() > self.limits.max_points_per_mutation {
            return Err(BridgeError::MutationTooLarge);
        }
        let name = collection_name(route)?;
        let schema = self
            .schemas
            .get(&name)
            .ok_or(BridgeError::CollectionNotFound)?
            .clone();
        let mut seen = BTreeSet::new();
        for point in &points {
            if !seen.insert(point.point_id) {
                return Err(BridgeError::DuplicatePointId);
            }
            validate_point(point, &schema, self.limits)?;
        }
        let mut vendor_points = Vec::with_capacity(points.len());
        for point in &points {
            vendor_points.push(PointStruct {
                id: Some(vendor_point_id(&point.point_id)),
                payload: encode_payload(point)?,
                vectors: Some(encode_vectors(point)),
            });
        }
        let acked = tokio::time::timeout(
            context.deadline(),
            self.client.upsert_points(UpsertPoints {
                collection_name: name.clone(),
                wait: Some(true),
                ordering: Some(strong_ordering()),
                points: vendor_points,
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?
        .map_err(map_mutation_error)?;
        if !acked
            .result
            .as_ref()
            .is_some_and(|result| update_completed(result.status))
        {
            return Err(BridgeError::MutationOutcomeUnknown);
        }
        context.check()?;
        let mut affected: Vec<QdrantPointId> = points.iter().map(|point| point.point_id).collect();
        affected.sort();
        self.verify_upsert_readback(&name, &points, &schema, context)
            .await?;
        self.record_mutation(route.clone(), mutation, affected)
    }

    async fn verify_upsert_readback(
        &self,
        name: &str,
        expected: &[PointRecord],
        schema: &CollectionSchema,
        context: &OpContext,
    ) -> Result<(), BridgeError> {
        let ids: Vec<PointId> = expected
            .iter()
            .map(|point| vendor_point_id(&point.point_id))
            .collect();
        let readback = tokio::time::timeout(
            context.deadline(),
            self.client.get_points(GetPoints {
                collection_name: name.to_owned(),
                ids,
                with_payload: Some(true.into()),
                with_vectors: Some(true.into()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?;
        if readback.result.len() != expected.len() {
            return Err(BridgeError::ExactReadbackMismatch);
        }
        for point in expected {
            let found = readback
                .result
                .iter()
                .find(|retrieved| {
                    retrieved.id.as_ref().is_some_and(|id| {
                        bridge_point_id(id).is_ok_and(|parsed| parsed == point.point_id)
                    })
                })
                .ok_or(BridgeError::ExactReadbackMismatch)?;
            let decoded = decode_point(
                found.id.as_ref().ok_or(BridgeError::MalformedResponse)?,
                &found.payload,
                found.vectors.as_ref(),
                schema,
            )
            .map_err(|_| BridgeError::ExactReadbackMismatch)?;
            if decoded != *point {
                return Err(BridgeError::ExactReadbackMismatch);
            }
        }
        Ok(())
    }

    /// Sets the exact exclusive upper epoch on explicit point IDs via
    /// payload-only update (no broad-filter closure exists). The current
    /// upper bound is read first, so a stale close fails pre-dispatch with
    /// [`BridgeError::ExactReadbackMismatch`] and a missing ID with
    /// [`BridgeError::PointNotFound`].
    pub async fn close_exact(
        &mut self,
        route: &CollectionRoute,
        ids: Vec<QdrantPointId>,
        valid_until_epoch_exclusive: search_contracts::Epoch,
        mutation: BridgeMutation,
        context: &OpContext,
    ) -> Result<MutationReceipt, BridgeError> {
        context.check()?;
        if let Some(replay) = self.replay(&mutation)? {
            return Ok(replay);
        }
        if self.operations.len() >= self.limits.max_operation_receipts {
            return Err(BridgeError::MutationTooLarge);
        }
        let ids = validate_exact_ids(ids, self.limits.max_points_per_mutation)?;
        let name = collection_name(route)?;
        let schema = self
            .schemas
            .get(&name)
            .ok_or(BridgeError::CollectionNotFound)?
            .clone();
        let current = self
            .fetch_points(&name, &ids, &schema, context)
            .await
            .map_err(|error| match error {
                BridgeError::TransportFailed => BridgeError::TransportFailed,
                BridgeError::CollectionNotFound => BridgeError::CollectionNotFound,
                _ => BridgeError::ExactReadbackMismatch,
            })?;
        for id in &ids {
            let point = current
                .iter()
                .find(|point| point.point_id == *id)
                .ok_or(BridgeError::PointNotFound)?;
            if valid_until_epoch_exclusive <= point.payload.valid_from_epoch {
                return Err(BridgeError::ExactReadbackMismatch);
            }
        }
        let mut payload = HashMap::new();
        payload.insert(
            EligibilityFilter::INDEXED_FIELDS[3].to_owned(),
            int_value(valid_until_epoch_exclusive.get()),
        );
        let acked = tokio::time::timeout(
            context.deadline(),
            self.client.set_payload(SetPayloadPoints {
                collection_name: name.clone(),
                wait: Some(true),
                payload,
                points_selector: Some(PointsSelector {
                    points_selector_one_of: Some(points_selector::PointsSelectorOneOf::Points(
                        PointsIdsList {
                            ids: ids.iter().map(vendor_point_id).collect(),
                        },
                    )),
                }),
                ordering: Some(strong_ordering()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?
        .map_err(map_mutation_error)?;
        if !acked
            .result
            .as_ref()
            .is_some_and(|result| update_completed(result.status))
        {
            return Err(BridgeError::MutationOutcomeUnknown);
        }
        context.check()?;
        let after = self
            .fetch_points(&name, &ids, &schema, context)
            .await
            .map_err(|_| BridgeError::MutationOutcomeUnknown)?;
        for id in &ids {
            let point = after
                .iter()
                .find(|point| point.point_id == *id)
                .ok_or(BridgeError::MutationOutcomeUnknown)?;
            if point.payload.valid_until_epoch_exclusive != Some(valid_until_epoch_exclusive) {
                return Err(BridgeError::ExactReadbackMismatch);
            }
        }
        self.record_mutation(route.clone(), mutation, ids)
    }

    /// Deletes only explicit exact point IDs with `wait=true` and strong
    /// ordering, then proves absence through exact readback.
    pub async fn delete_exact(
        &mut self,
        route: &CollectionRoute,
        ids: Vec<QdrantPointId>,
        mutation: BridgeMutation,
        context: &OpContext,
    ) -> Result<MutationReceipt, BridgeError> {
        context.check()?;
        if let Some(replay) = self.replay(&mutation)? {
            return Ok(replay);
        }
        if self.operations.len() >= self.limits.max_operation_receipts {
            return Err(BridgeError::MutationTooLarge);
        }
        let ids = validate_exact_ids(ids, self.limits.max_points_per_mutation)?;
        let name = collection_name(route)?;
        let acked = tokio::time::timeout(
            context.deadline(),
            self.client.delete_points(DeletePoints {
                collection_name: name.clone(),
                wait: Some(true),
                points: Some(PointsSelector {
                    points_selector_one_of: Some(points_selector::PointsSelectorOneOf::Points(
                        PointsIdsList {
                            ids: ids.iter().map(vendor_point_id).collect(),
                        },
                    )),
                }),
                ordering: Some(strong_ordering()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?
        .map_err(map_mutation_error)?;
        if !acked
            .result
            .as_ref()
            .is_some_and(|result| update_completed(result.status))
        {
            return Err(BridgeError::MutationOutcomeUnknown);
        }
        context.check()?;
        let present = tokio::time::timeout(
            context.deadline(),
            self.client.get_points(GetPoints {
                collection_name: name.clone(),
                ids: ids.iter().map(vendor_point_id).collect(),
                with_payload: Some(false.into()),
                with_vectors: Some(false.into()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?;
        if !present.result.is_empty() {
            return Err(BridgeError::ExactReadbackMismatch);
        }
        self.record_mutation(route.clone(), mutation, ids)
    }

    async fn fetch_points(
        &self,
        name: &str,
        ids: &[QdrantPointId],
        schema: &CollectionSchema,
        context: &OpContext,
    ) -> Result<Vec<PointRecord>, BridgeError> {
        let readback = tokio::time::timeout(
            context.deadline(),
            self.client.get_points(GetPoints {
                collection_name: name.to_owned(),
                ids: ids.iter().map(vendor_point_id).collect(),
                with_payload: Some(true.into()),
                with_vectors: Some(true.into()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::TransportFailed)?
        .map_err(map_read_error)?;
        let mut points = Vec::with_capacity(readback.result.len());
        for retrieved in &readback.result {
            points.push(decode_point(
                retrieved
                    .id
                    .as_ref()
                    .ok_or(BridgeError::MalformedResponse)?,
                &retrieved.payload,
                retrieved.vectors.as_ref(),
                schema,
            )?);
        }
        Ok(points)
    }
}
