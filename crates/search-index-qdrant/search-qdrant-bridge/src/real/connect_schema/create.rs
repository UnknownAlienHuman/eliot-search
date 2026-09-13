impl RealDataPlane {
    /// Creates one new opaque physical generation with mandatory payload
    /// indexes, strict-mode floors and post-creation schema verification.
    pub async fn create_collection(
        &mut self,
        route: &CollectionRoute,
        schema: &CollectionSchema,
        context: &OpContext,
    ) -> Result<ReceiptRef, BridgeError> {
        context.check()?;
        schema.validate()?;
        for vector_schema in schema.named_vectors.values() {
            if !vector_schema.sparse {
                return Err(BridgeError::CollectionSchemaMismatch);
            }
        }
        let name = collection_name(route)?;
        if self.schemas.contains_key(&name) {
            return Err(BridgeError::CollectionAlreadyExists);
        }
        if self
            .client
            .collection_exists(name.clone())
            .await
            .map_err(map_create_error)?
        {
            return Err(BridgeError::CollectionAlreadyExists);
        }
        let mut sparse = HashMap::new();
        for (vector_name, vector_schema) in &schema.named_vectors {
            sparse.insert(
                vector_name.clone(),
                SparseVectorParams {
                    modifier: if vector_schema.idf_enabled {
                        Some(Modifier::Idf as i32)
                    } else {
                        None
                    },
                    ..Default::default()
                },
            );
        }
        let create = CreateCollection {
            collection_name: name.clone(),
            shard_number: Some(1),
            replication_factor: Some(1),
            write_consistency_factor: Some(1),
            sparse_vectors_config: Some(SparseVectorConfig { map: sparse }),
            strict_mode_config: Some(StrictModeConfig {
                enabled: Some(true),
                unindexed_filtering_retrieve: Some(false),
                unindexed_filtering_update: Some(false),
                ..Default::default()
            }),
            ..Default::default()
        };
        let created = tokio::time::timeout(
            context.deadline(),
            self.client.create_collection(create),
        )
        .await
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?
        .map_err(map_create_error)?;
        if !created.result {
            return Err(BridgeError::MutationOutcomeUnknown);
        }
        for (field, field_type) in [
            (EligibilityFilter::INDEXED_FIELDS[0], FieldType::Keyword),
            (EligibilityFilter::INDEXED_FIELDS[1], FieldType::Keyword),
            (EligibilityFilter::INDEXED_FIELDS[2], FieldType::Integer),
            (EligibilityFilter::INDEXED_FIELDS[3], FieldType::Integer),
        ] {
            context.check()?;
            let indexed = tokio::time::timeout(
                context.deadline(),
                self.client.create_field_index(CreateFieldIndexCollection {
                    collection_name: name.clone(),
                    field_name: (*field).to_owned(),
                    field_type: Some(field_type as i32),
                    wait: Some(true),
                    ordering: Some(strong_ordering()),
                    ..Default::default()
                }),
            )
            .await
            .map_err(|_| BridgeError::TransportFailed)?
            .map_err(|_| BridgeError::TransportFailed)?;
            if !indexed
                .result
                .as_ref()
                .is_some_and(|result| update_completed(result.status))
            {
                return Err(BridgeError::TransportFailed);
            }
        }
        context.check()?;
        self.verify_server_schema(&name, schema, context).await?;
        self.schemas.insert(name.clone(), schema.clone());
        ReceiptRef::new(format!("qdrant:collection:{name}"))
            .map_err(|_| BridgeError::CollectionSchemaMismatch)
    }
}
