use std::collections::BTreeMap;

use search_contracts::ReceiptRef;

use crate::{
    BridgeError, CollectionRoute, CollectionSchema, PointRecord, QdrantBridge,
    QdrantPointId,
};

#[derive(Clone, Debug)]
pub(crate) struct CollectionState {
    pub(crate) schema: CollectionSchema,
    pub(crate) points: BTreeMap<QdrantPointId, PointRecord>,
}

impl QdrantBridge {
    /// Creates a new opaque physical generation after complete schema validation.
    pub fn create_candidate_collection(
        &mut self,
        route: CollectionRoute,
        schema: CollectionSchema,
    ) -> Result<ReceiptRef, BridgeError> {
        schema.validate()?;
        if self.collections.contains_key(&route) {
            return Err(BridgeError::CollectionAlreadyExists);
        }
        let receipt = ReceiptRef::new(format!(
            "qdrant:collection:{}",
            route.physical_name.as_str()
        ))
        .map_err(|_| BridgeError::CollectionSchemaMismatch)?;
        self.collections.insert(
            route,
            CollectionState {
                schema,
                points: BTreeMap::new(),
            },
        );
        Ok(receipt)
    }

    /// Verifies exact readback schema identity.
    pub fn verify_collection_schema(
        &self,
        route: &CollectionRoute,
        expected: &CollectionSchema,
    ) -> Result<ReceiptRef, BridgeError> {
        let actual = &self
            .collections
            .get(route)
            .ok_or(BridgeError::CollectionNotFound)?
            .schema;
        actual.validate()?;
        if actual != expected {
            return Err(BridgeError::CollectionSchemaMismatch);
        }
        ReceiptRef::new(format!("qdrant:schema:{}", route.physical_name.as_str()))
            .map_err(|_| BridgeError::CollectionSchemaMismatch)
    }
}
