impl RealDataPlane {
    /// Scrolls one bounded page of the exact filter population. The caller
    /// iterates with [`ScrollPage::next_offset`] and checks cancellation
    /// between pages; an empty page ends the walk with no continuation.
    pub async fn scroll_exact(
        &self,
        route: &CollectionRoute,
        filter: &EligibilityFilter,
        offset: Option<QdrantPointId>,
        limit: usize,
        context: &OpContext,
    ) -> Result<ScrollPage, BridgeError> {
        let budget = OperationBudget::begin(context)?;
        if filter.allowed_source_memberships.is_empty() {
            return Err(BridgeError::InvalidFilter);
        }
        if limit == 0 || limit > self.limits.max_query_candidates {
            return Err(BridgeError::QueryBudgetExceeded);
        }
        let name = collection_name(route)?;
        let schema = self
            .schemas
            .get(&name)
            .ok_or(BridgeError::CollectionNotFound)?;
        let vendor_filter = base_filter(filter)?;
        let scrolled = tokio::time::timeout(
            budget.remaining(context)?,
            self.client.scroll(ScrollPoints {
                collection_name: name,
                filter: Some(vendor_filter),
                offset: offset.as_ref().map(vendor_point_id),
                limit: Some(
                    u32::try_from(limit)
                        .map_err(|_| BridgeError::QueryBudgetExceeded)?,
                ),
                with_payload: Some(true.into()),
                with_vectors: Some(true.into()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::DeadlineExceeded)?
        .map_err(map_read_error)?;
        if scrolled.result.len() > limit {
            return Err(BridgeError::MalformedResponse);
        }
        let mut points = Vec::with_capacity(scrolled.result.len());
        let mut seen = BTreeSet::new();
        for retrieved in &scrolled.result {
            let point = decode_point(
                retrieved
                    .id
                    .as_ref()
                    .ok_or(BridgeError::MalformedResponse)?,
                &retrieved.payload,
                retrieved.vectors.as_ref(),
                schema,
            )?;
            if !seen.insert(point.point_id) || !filter.matches(&point.payload) {
                return Err(BridgeError::MalformedResponse);
            }
            points.push(point);
        }
        let next_offset = if scrolled.result.is_empty() {
            None
        } else {
            scrolled
                .next_page_offset
                .as_ref()
                .map(bridge_point_id)
                .transpose()?
        };
        if next_offset.is_some() && next_offset == offset {
            return Err(BridgeError::MalformedResponse);
        }
        Ok(ScrollPage {
            points,
            next_offset,
        })
    }
}
