impl RealDataPlane {
    async fn verify_server_schema(
        &self,
        name: &str,
        schema: &CollectionSchema,
        context: &OpContext,
    ) -> Result<(), BridgeError> {
        self.authorize_dispatch()?;
        let info = tokio::time::timeout(context.deadline(), self.client.collection_info(name))
            .await
            .map_err(|_| BridgeError::TransportFailed)?
            .map_err(map_read_error)?
            .result
            .ok_or(BridgeError::MalformedResponse)?;
        verify_server_schema(&info, schema)
    }

    /// Verifies exact readback schema identity and admits the managed route.
    ///
    /// A freshly connected adapter has no process-local schema cache. The
    /// expected schema therefore comes from the committed Search route/profile
    /// owner and is checked against the live server before it is retained for
    /// bounded query and mutation validation. This never reconstructs Search
    /// authority from Qdrant and never adopts a collection without an exact
    /// caller-supplied expectation.
    pub async fn verify_schema(
        &mut self,
        route: &CollectionRoute,
        expected: &CollectionSchema,
        context: &OpContext,
    ) -> Result<ReceiptRef, BridgeError> {
        context.check()?;
        expected.validate()?;
        let name = collection_name(route)?;
        if self
            .schemas
            .get(&name)
            .is_some_and(|actual| actual != expected)
        {
            return Err(BridgeError::CollectionSchemaMismatch);
        }
        let receipt = ReceiptRef::new(format!("qdrant:schema:{name}"))
            .map_err(|_| BridgeError::CollectionSchemaMismatch)?;
        self.verify_server_schema(&name, expected, context).await?;
        self.schemas.insert(name, expected.clone());
        Ok(receipt)
    }
}
