impl RealDataPlane {
    /// Counts the exact already-authorized filter population with
    /// `exact=true`. Unindexed or strict-rejected filters fail with
    /// [`BridgeError::UnindexedFilter`] rather than scanning.
    pub async fn count_exact(
        &self,
        route: &CollectionRoute,
        filter: &EligibilityFilter,
        context: &OpContext,
    ) -> Result<ExactCount, BridgeError> {
        context.check()?;
        if filter.allowed_source_memberships.is_empty() {
            return Err(BridgeError::InvalidFilter);
        }
        let name = collection_name(route)?;
        let vendor_filter = base_filter(filter)?;
        let counted = tokio::time::timeout(
            context.deadline(),
            self.client.count(CountPoints {
                collection_name: name,
                filter: Some(vendor_filter),
                exact: Some(true),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::TransportFailed)?
        .map_err(map_read_error)?;
        let count = counted
            .result
            .as_ref()
            .ok_or(BridgeError::MalformedResponse)?
            .count;
        Ok(ExactCount {
            count: usize::try_from(count)
                .map_err(|_| BridgeError::MalformedResponse)?,
        })
    }
}
