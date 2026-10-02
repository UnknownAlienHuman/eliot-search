fn post_create_check(
    context: &OpContext,
    budget: &OperationBudget,
) -> Result<(), BridgeError> {
    budget.remaining_after_dispatch(context).map(|_| ())
}

const fn map_post_create_error(error: BridgeError) -> BridgeError {
    match error {
        BridgeError::Cancelled
        | BridgeError::DeadlineExceeded
        | BridgeError::TransportFailed
        | BridgeError::MalformedResponse => BridgeError::MutationOutcomeUnknown,
        other => other,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PayloadIndexKind {
    Uuid,
    Keyword,
    Integer,
}

pub(super) const fn payload_index_specs() -> [(&'static str, PayloadIndexKind); 19] {
    [
        (PointPayload::INSTALLATION_INCARNATION_FIELD, PayloadIndexKind::Uuid),
        (PointPayload::COLLECTION_GENERATION_FIELD, PayloadIndexKind::Uuid),
        (PointPayload::PROJECTION_MEMBERSHIP_FIELD, PayloadIndexKind::Uuid),
        (PointPayload::ACCESS_PARTITION_FIELD, PayloadIndexKind::Uuid),
        (PointPayload::SCORING_PARTITION_FIELD, PayloadIndexKind::Uuid),
        (PointPayload::SOURCE_ID_FIELD, PayloadIndexKind::Uuid),
        (PointPayload::SOURCE_REVISION_FIELD, PayloadIndexKind::Uuid),
        (PointPayload::REPRESENTATION_FIELD, PayloadIndexKind::Uuid),
        (PointPayload::UNIT_FIELD, PayloadIndexKind::Uuid),
        (PointPayload::SCORING_DOCUMENT_FIELD, PayloadIndexKind::Uuid),
        (PointPayload::PROJECTION_PROFILE_SET_FIELD, PayloadIndexKind::Keyword),
        (PointPayload::UNIT_KIND_FIELD, PayloadIndexKind::Keyword),
        (PointPayload::MODALITY_FIELD, PayloadIndexKind::Keyword),
        (PointPayload::LANGUAGE_OR_FORMAT_FIELD, PayloadIndexKind::Keyword),
        (PointPayload::ENTITY_KIND_FIELD, PayloadIndexKind::Keyword),
        (PointPayload::NORMALIZED_SYMBOL_FIELD, PayloadIndexKind::Keyword),
        (PointPayload::REPOSITORY_LINEAGE_FIELD, PayloadIndexKind::Uuid),
        (PointPayload::VALID_FROM_FIELD, PayloadIndexKind::Integer),
        (PointPayload::VALID_UNTIL_FIELD, PayloadIndexKind::Integer),
    ]
}

const fn vendor_field_type(kind: PayloadIndexKind) -> FieldType {
    match kind {
        PayloadIndexKind::Uuid => FieldType::Uuid,
        PayloadIndexKind::Keyword => FieldType::Keyword,
        PayloadIndexKind::Integer => FieldType::Integer,
    }
}

pub(super) const fn vendor_schema_type(kind: PayloadIndexKind) -> PayloadSchemaType {
    match kind {
        PayloadIndexKind::Uuid => PayloadSchemaType::Uuid,
        PayloadIndexKind::Keyword => PayloadSchemaType::Keyword,
        PayloadIndexKind::Integer => PayloadSchemaType::Integer,
    }
}

fn strict_mode_config(enabled: bool) -> StrictModeConfig {
    StrictModeConfig {
        enabled: Some(enabled),
        unindexed_filtering_retrieve: Some(false),
        unindexed_filtering_update: Some(false),
        ..Default::default()
    }
}

impl RealDataPlane {
    /// Creates one new opaque physical generation in the required order:
    /// collection without strict admission, every S9.5 payload index, strict
    /// mode enablement, then exact live schema verification.
    ///
    /// Before the collection create dispatch, cancellation or deadline expiry
    /// is definite. Once the collection may exist, cancellation, timeout,
    /// transport loss or unusable readback reports
    /// [`BridgeError::MutationOutcomeUnknown`] because a partial schema may be
    /// durable and must be reconciled explicitly.
    pub async fn create_collection(
        &mut self,
        route: &CollectionRoute,
        schema: &CollectionSchema,
        context: &OpContext,
    ) -> Result<ReceiptRef, BridgeError> {
        let budget = OperationBudget::begin(context)?;
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
        let exists = tokio::time::timeout(
            budget.remaining(context)?,
            self.client.collection_exists(name.clone()),
        )
        .await
        .map_err(|_| BridgeError::DeadlineExceeded)?
        .map_err(map_read_error)?;
        if exists {
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
            // Strict admission is deliberately disabled until every mandatory
            // payload index has been acknowledged.
            strict_mode_config: Some(strict_mode_config(false)),
            ..Default::default()
        };
        let created = tokio::time::timeout(
            budget.remaining(context)?,
            self.client.create_collection(create),
        )
        .await
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?
        .map_err(map_create_error)?;
        if !created.result {
            return Err(BridgeError::MutationOutcomeUnknown);
        }

        for (field, field_type) in payload_index_specs() {
            post_create_check(context, &budget)?;
            let indexed = tokio::time::timeout(
                budget.remaining_after_dispatch(context)?,
                self.client.create_field_index(CreateFieldIndexCollection {
                    collection_name: name.clone(),
                    field_name: field.to_owned(),
                    field_type: Some(vendor_field_type(field_type) as i32),
                    wait: Some(true),
                    ordering: Some(strong_ordering()),
                    ..Default::default()
                }),
            )
            .await
            .map_err(|_| BridgeError::MutationOutcomeUnknown)?
            .map_err(map_create_error)
            .map_err(map_post_create_error)?;
            if !indexed
                .result
                .as_ref()
                .is_some_and(|result| update_completed(result.status))
            {
                return Err(BridgeError::MutationOutcomeUnknown);
            }
        }

        post_create_check(context, &budget)?;
        let hardened = tokio::time::timeout(
            budget.remaining_after_dispatch(context)?,
            self.client.update_collection(UpdateCollection {
                collection_name: name.clone(),
                strict_mode_config: Some(strict_mode_config(true)),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?
        .map_err(map_create_error)
        .map_err(map_post_create_error)?;
        if !hardened.result {
            return Err(BridgeError::MutationOutcomeUnknown);
        }

        post_create_check(context, &budget)?;
        self.verify_server_schema(
            &name,
            schema,
            budget.remaining_after_dispatch(context)?,
        )
        .await
        .map_err(map_post_create_error)?;
        self.schemas.insert(name.clone(), schema.clone());
        ReceiptRef::new(format!("qdrant:collection:{name}"))
            .map_err(|_| BridgeError::CollectionSchemaMismatch)
    }
}

#[cfg(test)]
mod create_tests {
    use super::*;

    #[test]
    fn possible_collection_effects_never_return_definite_no_write_errors() {
        for error in [
            BridgeError::Cancelled,
            BridgeError::DeadlineExceeded,
            BridgeError::TransportFailed,
            BridgeError::MalformedResponse,
        ] {
            assert_eq!(
                map_post_create_error(error),
                BridgeError::MutationOutcomeUnknown
            );
        }
        assert_eq!(
            map_post_create_error(BridgeError::CollectionSchemaMismatch),
            BridgeError::CollectionSchemaMismatch
        );
        assert_eq!(
            map_post_create_error(BridgeError::AuthenticationInvalid),
            BridgeError::AuthenticationInvalid
        );
    }

    #[test]
    fn index_plan_and_strict_mode_floors_are_explicit() {
        let specs = payload_index_specs();
        assert_eq!(specs.len(), PointPayload::INDEXED_FIELDS.len());
        assert!(specs.contains(&(
            PointPayload::INSTALLATION_INCARNATION_FIELD,
            PayloadIndexKind::Uuid,
        )));
        assert!(specs.contains(&(
            PointPayload::PROJECTION_PROFILE_SET_FIELD,
            PayloadIndexKind::Keyword,
        )));
        assert!(specs.contains(&(
            PointPayload::VALID_UNTIL_FIELD,
            PayloadIndexKind::Integer,
        )));

        let staging = strict_mode_config(false);
        assert_eq!(staging.enabled, Some(false));
        assert_eq!(staging.unindexed_filtering_retrieve, Some(false));
        assert_eq!(staging.unindexed_filtering_update, Some(false));

        let admitted = strict_mode_config(true);
        assert_eq!(admitted.enabled, Some(true));
        assert_eq!(admitted.unindexed_filtering_retrieve, Some(false));
        assert_eq!(admitted.unindexed_filtering_update, Some(false));
    }
}
