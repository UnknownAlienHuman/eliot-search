impl RealDataPlane {
    /// Reads back exactly the requested identifiers with explicit
    /// missing/unexpected sets.
    pub async fn readback_exact(
        &self,
        route: &CollectionRoute,
        ids: Vec<QdrantPointId>,
        context: &OpContext,
    ) -> Result<BoundedPointReadback, BridgeError> {
        context.check()?;
        let ids = validate_exact_ids(ids, self.limits.max_points_per_mutation)?;
        let name = collection_name(route)?;
        let schema = self
            .schemas
            .get(&name)
            .ok_or(BridgeError::CollectionNotFound)?;
        let readback = tokio::time::timeout(
            context.deadline(),
            self.client.get_points(GetPoints {
                collection_name: name.clone(),
                ids: ids.iter().map(vendor_point_id).collect(),
                with_payload: Some(true.into()),
                with_vectors: Some(true.into()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::TransportFailed)?
        .map_err(map_read_error)?;
        if readback.result.len() > ids.len() {
            return Err(BridgeError::MalformedResponse);
        }
        let requested: BTreeSet<QdrantPointId> = ids.into_iter().collect();
        let mut points = Vec::new();
        let mut seen = BTreeSet::new();
        let mut unexpected_ids = Vec::new();
        for retrieved in &readback.result {
            let id = bridge_point_id(
                retrieved
                    .id
                    .as_ref()
                    .ok_or(BridgeError::MalformedResponse)?,
            )?;
            if !requested.contains(&id) {
                unexpected_ids.push(id);
                continue;
            }
            seen.insert(id);
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
        let missing_ids: Vec<QdrantPointId> = requested.difference(&seen).copied().collect();
        Ok(BoundedPointReadback {
            points,
            missing_ids,
            unexpected_ids,
        })
    }

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
            count: usize::try_from(count).map_err(|_| BridgeError::MalformedResponse)?,
        })
    }

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
        let vendor_filter = base_filter(filter)?;
        let scrolled = tokio::time::timeout(
            context.deadline(),
            self.client.scroll(ScrollPoints {
                collection_name: name,
                filter: Some(vendor_filter),
                offset: offset.as_ref().map(vendor_point_id),
                limit: Some(u32::try_from(limit).map_err(|_| BridgeError::QueryBudgetExceeded)?),
                with_payload: Some(true.into()),
                with_vectors: Some(true.into()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::TransportFailed)?
        .map_err(map_read_error)?;
        if scrolled.result.len() > limit {
            return Err(BridgeError::MalformedResponse);
        }
        let mut points = Vec::with_capacity(scrolled.result.len());
        for retrieved in &scrolled.result {
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
        let next_offset = if scrolled.result.is_empty() {
            None
        } else {
            scrolled
                .next_page_offset
                .as_ref()
                .map(bridge_point_id)
                .transpose()?
        };
        Ok(ScrollPage {
            points,
            next_offset,
        })
    }

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
                query: Some(Query::new_nearest(VectorInput::new_sparse(indices, values))),
                using: Some(vector_name.to_owned()),
                filter: Some(vendor_filter),
                params: Some(SearchParams {
                    exact: Some(true),
                    idf: corpus.map(|population| IdfParams {
                        corpus: Some(population),
                    }),
                    ..Default::default()
                }),
                limit: Some(u64::try_from(limit).map_err(|_| BridgeError::QueryBudgetExceeded)?),
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
        for scored in &answered.result {
            if !scored.score.is_finite() {
                return Err(BridgeError::InvalidScore);
            }
            let point_id =
                bridge_point_id(scored.id.as_ref().ok_or(BridgeError::MalformedResponse)?)?;
            let payload_digest = hex_to_32(&get_string(&scored.payload, "payload_digest")?)?;
            let identity_digest = hex_to_32(&get_string(&scored.payload, "identity_digest")?)?;
            nominations.push(CandidateNomination {
                point_id,
                score: scored.score,
                payload_digest: search_contracts::Blake3Digest32::from_bytes(payload_digest),
                identity_digest: search_contracts::Blake3Digest32::from_bytes(identity_digest),
            });
        }
        Ok(nominations)
    }
}
