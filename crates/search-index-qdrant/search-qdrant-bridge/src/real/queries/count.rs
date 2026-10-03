impl RealDataPlane {
    /// Counts the exact already-authorized filter population with
    /// `exact=true`. Unindexed or strict-rejected filters fail with
    /// [`BridgeError::UnindexedFilter`] rather than scanning. The exact route
    /// generation must have passed live schema admission in this process.
    pub async fn count_exact(
        &self,
        route: &CollectionRoute,
        filter: &EligibilityFilter,
        context: &OpContext,
    ) -> Result<ExactCount, BridgeError> {
        let budget = OperationBudget::begin(context)?;
        validate_filter_for_route(filter, route)?;
        let (name, schema) = self.admitted_schema(route)?;
        ensure_filter_indexes(schema)?;
        let vendor_filter = base_filter(filter)?;
        let counted = tokio::time::timeout(
            budget.remaining(context)?,
            self.client.count(CountPoints {
                collection_name: name,
                filter: Some(vendor_filter),
                exact: Some(true),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::DeadlineExceeded)?
        .map_err(map_read_error)?;
        let count = counted
            .result
            .as_ref()
            .ok_or(BridgeError::MalformedResponse)?
            .count;
        Ok(ExactCount {
            count: usize::try_from(count).map_err(|_| BridgeError::MalformedResponse)?,
        })
    }
}
