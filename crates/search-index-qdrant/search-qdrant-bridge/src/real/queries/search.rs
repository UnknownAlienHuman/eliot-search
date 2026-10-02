impl RealDataPlane {
    /// Returns bounded filtered nominations for one already-authorized leg.
    /// Retrieval and `idf.corpus` are rendered from the single `filter`
    /// contract: [`IdfScope::ScopedToRetrieval`] clones the retrieval filter
    /// as the corpus, [`IdfScope::Global`] omits it. Scores are finite
    /// nominations, never evidence.
    pub async fn query_filtered(
        &self,
        route: &CollectionRoute,
        filter: &EligibilityFilter,
        vector_name: &str,
        query: &[(u32, f32)],
        limit: usize,
        idf: IdfScope,
        context: &OpContext,
    ) -> Result<Vec<CandidateNomination>, BridgeError> {
        context.check()?;
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
        let vector_schema = schema
            .named_vectors
            .get(vector_name)
            .ok_or(BridgeError::NamedVectorMissing)?;
        validate_query_vector(query, vector_schema.dimensions)?;
        let vendor_filter = base_filter(filter)?;
        let corpus = match idf {
            IdfScope::Global => None,
            IdfScope::ScopedToRetrieval => Some(vendor_filter.clone()),
        };
        let (indices, values): (Vec<u32>, Vec<f32>) = query.iter().copied().unzip();
        let answered = tokio::time::timeout(
            context.deadline(),
            self.client.query(QueryPoints {
                collection_name: name,
                query: Some(Query::new_nearest(VectorInput::new_sparse(
                    indices, values,
                ))),
                using: Some(vector_name.to_owned()),
                filter: Some(vendor_filter),
                params: Some(SearchParams {
                    exact: Some(true),
                    idf: corpus.map(|population| IdfParams {
                        corpus: Some(population),
                    }),
                    ..Default::default()
                }),
                limit: Some(
                    u64::try_from(limit)
                        .map_err(|_| BridgeError::QueryBudgetExceeded)?,
                ),
                with_payload: Some(true.into()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::TransportFailed)?
        .map_err(map_read_error)?;
        if answered.result.len() > limit {
            return Err(BridgeError::MalformedResponse);
        }
        let mut nominations = Vec::with_capacity(answered.result.len());
        let mut seen = BTreeSet::new();
        for scored in &answered.result {
            if !scored.score.is_finite() {
                return Err(BridgeError::InvalidScore);
            }
            let point_id = bridge_point_id(
                scored
                    .id
                    .as_ref()
                    .ok_or(BridgeError::MalformedResponse)?,
            )?;
            if !seen.insert(point_id) {
                return Err(BridgeError::MalformedResponse);
            }
            let payload = decode_payload(&scored.payload)?;
            if !filter.matches(&payload) {
                // An off-filter answer may already have displaced an eligible
                // point from top-k. Reject the whole leg; never post-filter it
                // into an apparently valid partial ranking.
                return Err(BridgeError::MalformedResponse);
            }
            nominations.push(CandidateNomination {
                point_id,
                score: scored.score,
                payload_digest: payload.payload_digest,
                identity_digest: payload.identity_digest,
            });
        }
        // Qdrant returns a top-k set; daemon composition receives one stable
        // bridge order independent of vendor tie ordering.
        nominations.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(core::cmp::Ordering::Equal)
                .then_with(|| left.point_id.cmp(&right.point_id))
        });
        Ok(nominations)
    }
}
