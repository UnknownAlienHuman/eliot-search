impl RealDataPlane {
    async fn verify_server_schema(
        &self,
        name: &str,
        schema: &CollectionSchema,
        context: &OpContext,
    ) -> Result<(), BridgeError> {
        let info = tokio::time::timeout(
            context.deadline(),
            self.client.collection_info(name),
        )
        .await
        .map_err(|_| BridgeError::TransportFailed)?
        .map_err(map_read_error)?
        .result
        .ok_or(BridgeError::MalformedResponse)?;
        verify_server_schema(&info, schema)
    }

    /// Verifies exact readback schema identity for a managed collection.
    pub async fn verify_schema(
        &self,
        route: &CollectionRoute,
        expected: &CollectionSchema,
        context: &OpContext,
    ) -> Result<ReceiptRef, BridgeError> {
        context.check()?;
        expected.validate()?;
        let name = collection_name(route)?;
        let actual = self
            .schemas
            .get(&name)
            .ok_or(BridgeError::CollectionNotFound)?;
        actual.validate()?;
        if actual != expected {
            return Err(BridgeError::CollectionSchemaMismatch);
        }
        self.verify_server_schema(&name, expected, context).await?;
        ReceiptRef::new(format!("qdrant:schema:{name}"))
            .map_err(|_| BridgeError::CollectionSchemaMismatch)
    }
}
