//! Exact S9.5/S11 mutation and full readback path.
//!
//! This module has no replay ledger. Durable intent and replay classification
//! belong to the publication/control owner. Every preflight and post-effect
//! read uses exact IDs without an eligibility filter, so staged and closed
//! points remain visible to collision checks.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use qdrant_client::qdrant::{
    DeletePoints, GetPoints, NamedVectors as VendorNamedVectors, PointStruct, PointsIdsList,
    PointsSelector, RetrievedPoint as VendorRetrievedPoint, SetPayloadPoints, UpsertPoints,
    Vector as VendorVector, Vectors as VendorVectors, VectorsOutput as VendorVectorsOutput,
    points_selector, vector_output as vendor_vector_output, vectors as vendor_vectors,
    vectors_output as vendor_vectors_output,
};
use search_contracts::Epoch;
use search_contracts::indexed::{
    CollectionSchema as IndexedCollectionSchema, MAX_STORED_VECTOR_VALUES, NamedVectorValue,
    PayloadField, PointReadback, VectorRequirement, VectorValue,
};
use search_point_identity::s11::PointIdentity;

use super::canonical::{
    CanonicalAdmission, CanonicalRequestContext, canonical_bridge_point_id, canonical_epoch_bound,
    canonical_vendor_point_id, decode_payload, encode_payload, verify_observed_identity,
};
use super::{BridgeError, CollectionRoute, RealDataPlane};

/// Caller-owned exact point expectation from the accepted projection plan.
/// The identity and payload/vector contracts stay shared; this wrapper adds no
/// point-ID or digest derivation.
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalPointInput {
    /// Genuine S11 identity, including its complete canonical key.
    pub identity: PointIdentity,
    /// Expected closed S9.5 payload and complete named-vector values.
    pub point: PointReadback,
}

/// Actual full payload/vector readback for one exact S11 address.
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalMutationPointReadback {
    /// Address parsed from the Qdrant response.
    pub address: search_point_identity::s11::PointId128,
    /// Actual accepted payload and all named vectors returned by Qdrant.
    pub point: PointReadback,
}

/// Bounded exact readback; absence remains explicit.
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalMutationReadback {
    /// Points returned in ascending S11 address order.
    pub points: Vec<CanonicalMutationPointReadback>,
    /// Requested addresses absent from Qdrant.
    pub missing_ids: Vec<search_point_identity::s11::PointId128>,
}

/// Exact deletion postcondition observed through point-ID readback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalDeleteReadback {
    /// Requested addresses confirmed absent after the acknowledged delete.
    pub absent_ids: Vec<search_point_identity::s11::PointId128>,
}

fn admission_for<'a>(
    plane: &'a RealDataPlane,
    route: &CollectionRoute,
) -> Result<&'a CanonicalAdmission, BridgeError> {
    let admission = plane.canonical.resolve(route)?;
    if route.generation != *admission.schema.collection_generation_id() {
        return Err(BridgeError::CollectionSchemaMismatch);
    }
    Ok(admission)
}

fn validate_point_readback(
    point: &PointReadback,
    schema: &IndexedCollectionSchema,
    max_values_per_point: usize,
) -> Result<(), BridgeError> {
    point
        .payload
        .validate()
        .map_err(|_| BridgeError::CollectionSchemaMismatch)?;
    if point.vectors.len() != schema.vectors().len() {
        return Err(BridgeError::VectorDimensionMismatch);
    }

    let contract_limit =
        usize::try_from(MAX_STORED_VECTOR_VALUES).map_err(|_| BridgeError::MutationTooLarge)?;
    let value_limit = contract_limit.min(max_values_per_point);
    let mut total_values = 0_usize;
    let mut seen = BTreeSet::new();
    for vector in &point.vectors {
        if !seen.insert(vector.name.clone()) {
            return Err(BridgeError::DuplicatePointId);
        }
        let requirement = schema
            .vectors()
            .iter()
            .find(|required| required.name() == &vector.name)
            .ok_or(BridgeError::NamedVectorMissing)?;
        if !matches!(requirement.requirement(), VectorRequirement::Sparse { .. }) {
            return Err(BridgeError::UnsupportedVectorProfile);
        }
        vector
            .value
            .validate(&requirement.requirement())
            .map_err(|_| BridgeError::VectorDimensionMismatch)?;
        total_values = total_values
            .checked_add(vector.value.stored_values())
            .filter(|total| *total <= value_limit)
            .ok_or(BridgeError::MutationTooLarge)?;
    }
    Ok(())
}

fn validate_point_input(
    route: &CollectionRoute,
    schema: &IndexedCollectionSchema,
    input: &CanonicalPointInput,
    max_values_per_point: usize,
) -> Result<(), BridgeError> {
    validate_point_readback(&input.point, schema, max_values_per_point)?;
    let key = input.identity.key();
    let payload = &input.point.payload;
    let s11_coordinates_match = [
        key.installation_incarnation_id == payload.installation_incarnation_id,
        key.collection_generation_id == payload.collection_generation_id,
        key.projection_membership_id == payload.projection_membership_id,
        key.representation_id == payload.representation_id,
        key.unit_id == payload.unit_id,
        key.projection_profile_set_id == payload.projection_profile_set_id,
    ]
    .into_iter()
    .all(|matches| matches);
    let route_schema_match = payload.collection_generation_id == route.generation
        && payload.installation_incarnation_id == *schema.installation_incarnation_id()
        && payload.projection_profile_set_id == *schema.projection_profile_set_id();
    if !s11_coordinates_match || !route_schema_match {
        return Err(BridgeError::CollectionSchemaMismatch);
    }
    if input.identity.full_digest() != payload.point_identity_digest_256 {
        return Err(BridgeError::CollectionSchemaMismatch);
    }
    verify_observed_identity(&input.identity, input.identity.point_id(), payload)
}

fn ordered_inputs<'a>(
    route: &CollectionRoute,
    admission: &CanonicalAdmission,
    inputs: &'a [CanonicalPointInput],
    limits: crate::BridgeLimits,
    context: &CanonicalRequestContext,
) -> Result<Vec<&'a CanonicalPointInput>, BridgeError> {
    if inputs.is_empty() || inputs.len() > limits.max_points_per_mutation {
        return Err(BridgeError::MutationTooLarge);
    }
    let mut ordered: Vec<_> = inputs.iter().collect();
    ordered.sort_unstable_by_key(|input| input.identity.point_id());
    let mut previous = None;
    for input in &ordered {
        context.remaining()?;
        let point_id = input.identity.point_id();
        if previous == Some(point_id) {
            return Err(BridgeError::DuplicatePointId);
        }
        previous = Some(point_id);
        validate_point_input(
            route,
            &admission.schema,
            input,
            limits.max_vector_values_per_point,
        )?;
    }
    context.remaining()?;
    Ok(ordered)
}

fn floats_equal_exact(left: &[f32], right: &[f32]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.to_bits() == right.to_bits())
}

fn vector_equal_exact(left: &VectorValue, right: &VectorValue) -> bool {
    match (left, right) {
        (VectorValue::Dense(left), VectorValue::Dense(right)) => floats_equal_exact(left, right),
        (
            VectorValue::Sparse {
                indices: left_indices,
                values: left_values,
            },
            VectorValue::Sparse {
                indices: right_indices,
                values: right_values,
            },
        ) => left_indices == right_indices && floats_equal_exact(left_values, right_values),
        _ => false,
    }
}

fn point_readback_equal_exact(left: &PointReadback, right: &PointReadback) -> bool {
    left.payload == right.payload
        && left.vectors.len() == right.vectors.len()
        && left.vectors.iter().all(|expected| {
            right
                .vectors
                .iter()
                .find(|actual| actual.name == expected.name)
                .is_some_and(|actual| vector_equal_exact(&expected.value, &actual.value))
        })
}

fn point_readback_equal_for_close(left: &PointReadback, right: &PointReadback) -> bool {
    let mut left_payload = left.payload.clone();
    let mut right_payload = right.payload.clone();
    left_payload.valid_until_epoch_exclusive = None;
    right_payload.valid_until_epoch_exclusive = None;
    left_payload == right_payload
        && left.vectors.len() == right.vectors.len()
        && left.vectors.iter().all(|expected| {
            right
                .vectors
                .iter()
                .find(|actual| actual.name == expected.name)
                .is_some_and(|actual| vector_equal_exact(&expected.value, &actual.value))
        })
}

fn encode_named_vectors(point: &PointReadback) -> Result<VendorVectors, BridgeError> {
    let mut vectors = HashMap::with_capacity(point.vectors.len());
    for named in &point.vectors {
        let vector = match &named.value {
            VectorValue::Sparse { indices, values } => {
                VendorVector::new_sparse(indices.clone(), values.clone())
            }
            VectorValue::Dense(_) => return Err(BridgeError::UnsupportedVectorProfile),
        };
        vectors.insert(named.name.as_str().to_owned(), vector);
    }
    Ok(VendorVectors {
        vectors_options: Some(vendor_vectors::VectorsOptions::Vectors(
            VendorNamedVectors { vectors },
        )),
    })
}

fn decode_named_vectors(
    vectors: Option<&VendorVectorsOutput>,
    schema: &IndexedCollectionSchema,
    max_values_per_point: usize,
) -> Result<Vec<NamedVectorValue>, BridgeError> {
    let named = match vectors.and_then(|vectors| vectors.vectors_options.as_ref()) {
        Some(vendor_vectors_output::VectorsOptions::Vectors(named)) => &named.vectors,
        _ => return Err(BridgeError::MalformedResponse),
    };
    if named.len() != schema.vectors().len() {
        return Err(BridgeError::MalformedResponse);
    }
    let contract_limit =
        usize::try_from(MAX_STORED_VECTOR_VALUES).map_err(|_| BridgeError::MalformedResponse)?;
    let value_limit = contract_limit.min(max_values_per_point);
    let mut total_values = 0_usize;
    let mut decoded = Vec::with_capacity(schema.vectors().len());
    for requirement in schema.vectors() {
        let entry = named
            .get(requirement.name().as_str())
            .ok_or(BridgeError::MalformedResponse)?;
        let value = match (requirement.requirement(), entry.vector.as_ref()) {
            (
                VectorRequirement::Sparse { .. },
                Some(vendor_vector_output::Vector::Sparse(sparse)),
            ) => VectorValue::Sparse {
                indices: sparse.indices.clone(),
                values: sparse.values.clone(),
            },
            (VectorRequirement::Dense { .. }, _) => {
                return Err(BridgeError::UnsupportedVectorProfile);
            }
            _ => return Err(BridgeError::MalformedResponse),
        };
        value
            .validate(&requirement.requirement())
            .map_err(|_| BridgeError::MalformedResponse)?;
        total_values = total_values
            .checked_add(value.stored_values())
            .filter(|total| *total <= value_limit)
            .ok_or(BridgeError::MalformedResponse)?;
        decoded.push(NamedVectorValue {
            name: requirement.name().clone(),
            value,
        });
    }
    Ok(decoded)
}

fn decode_full_point(
    retrieved: &VendorRetrievedPoint,
    schema: &IndexedCollectionSchema,
    max_values_per_point: usize,
) -> Result<CanonicalMutationPointReadback, BridgeError> {
    let point_id = canonical_bridge_point_id(
        retrieved
            .id
            .as_ref()
            .ok_or(BridgeError::MalformedResponse)?,
    )?;
    let point = PointReadback {
        payload: decode_payload(&retrieved.payload)?,
        vectors: decode_named_vectors(retrieved.vectors.as_ref(), schema, max_values_per_point)?,
    };
    validate_point_readback(&point, schema, max_values_per_point)
        .map_err(|_| BridgeError::MalformedResponse)?;
    Ok(CanonicalMutationPointReadback {
        address: point_id,
        point,
    })
}

fn validate_close_epoch(
    existing: &search_contracts::indexed::PointPayload,
    requested: Epoch,
) -> Result<(), BridgeError> {
    canonical_epoch_bound(requested.get())?;
    if requested <= existing.valid_from_epoch {
        return Err(BridgeError::ExactReadbackMismatch);
    }
    if let Some(current) = existing.valid_until_epoch_exclusive {
        canonical_epoch_bound(current.get())?;
        if current != requested {
            return Err(BridgeError::ExactReadbackMismatch);
        }
    }
    Ok(())
}

async fn fetch_full_readback(
    plane: &RealDataPlane,
    admission: &CanonicalAdmission,
    ordered: &[&CanonicalPointInput],
    context: &CanonicalRequestContext,
) -> Result<CanonicalMutationReadback, BridgeError> {
    let mut expected_by_id = BTreeMap::new();
    for input in ordered {
        expected_by_id.insert(input.identity.point_id(), *input);
    }
    let ids = ordered
        .iter()
        .map(|input| canonical_vendor_point_id(input.identity.point_id()))
        .collect();
    context.remaining()?;
    plane.authorize_dispatch()?;
    let response = tokio::time::timeout(
        context.remaining()?,
        plane.client.get_points(GetPoints {
            collection_name: admission.collection_name.clone(),
            ids,
            with_payload: Some(true.into()),
            with_vectors: Some(true.into()),
            ..Default::default()
        }),
    )
    .await
    .map_err(|_| BridgeError::TransportFailed)?
    .map_err(super::map_read_error)?;
    context.remaining()?;
    if response.result.len() > ordered.len() {
        return Err(BridgeError::MalformedResponse);
    }

    let mut returned = BTreeMap::new();
    for retrieved in &response.result {
        context.remaining()?;
        let decoded = decode_full_point(
            retrieved,
            &admission.schema,
            plane.limits.max_vector_values_per_point,
        )?;
        let expected = expected_by_id
            .get(&decoded.address)
            .ok_or(BridgeError::UnexpectedPoint)?;
        verify_observed_identity(&expected.identity, decoded.address, &decoded.point.payload)?;
        if returned.insert(decoded.address, decoded).is_some() {
            return Err(BridgeError::MalformedResponse);
        }
    }

    let mut points = Vec::with_capacity(returned.len());
    let mut missing_ids = Vec::new();
    for input in ordered {
        context.remaining()?;
        let point_id = input.identity.point_id();
        match returned.remove(&point_id) {
            Some(point) => points.push(point),
            None => missing_ids.push(point_id),
        }
    }
    context.remaining()?;
    Ok(CanonicalMutationReadback {
        points,
        missing_ids,
    })
}

impl RealDataPlane {
    /// Reads exact canonical addresses without an eligibility filter. The
    /// caller's full S11 identity, payload and named-vector expectation are
    /// checked against actual Qdrant readback; missing IDs stay explicit.
    pub async fn readback_indexed_points_exact(
        &self,
        route: &CollectionRoute,
        expected: &[CanonicalPointInput],
        context: &CanonicalRequestContext,
    ) -> Result<CanonicalMutationReadback, BridgeError> {
        let admission = admission_for(self, route)?;
        let ordered = ordered_inputs(route, admission, expected, self.limits, context)?;
        let readback = fetch_full_readback(self, admission, &ordered, context).await?;
        let expected_by_id: BTreeMap<_, _> = ordered
            .iter()
            .map(|input| (input.identity.point_id(), *input))
            .collect();
        for actual in &readback.points {
            context.remaining()?;
            let expected = expected_by_id
                .get(&actual.address)
                .ok_or(BridgeError::UnexpectedPoint)?;
            if !point_readback_equal_exact(&actual.point, &expected.point) {
                return Err(BridgeError::ExactReadbackMismatch);
            }
        }
        context.remaining()?;
        Ok(readback)
    }

    /// Upserts a bounded exact point set only after full S11 collision and
    /// payload/vector preflight. Success contains actual Qdrant readback.
    pub async fn upsert_indexed_exact(
        &mut self,
        route: &CollectionRoute,
        points: &[CanonicalPointInput],
        context: &CanonicalRequestContext,
    ) -> Result<CanonicalMutationReadback, BridgeError> {
        let admission = admission_for(self, route)?;
        let ordered = ordered_inputs(route, admission, points, self.limits, context)?;

        let preflight = fetch_full_readback(self, admission, &ordered, context).await?;
        let expected_by_id: BTreeMap<_, _> = ordered
            .iter()
            .map(|input| (input.identity.point_id(), *input))
            .collect();
        for existing in &preflight.points {
            context.remaining()?;
            let expected = expected_by_id
                .get(&existing.address)
                .ok_or(BridgeError::UnexpectedPoint)?;
            if !point_readback_equal_exact(&existing.point, &expected.point) {
                return Err(BridgeError::ExactReadbackMismatch);
            }
        }

        let mut vendor_points = Vec::with_capacity(ordered.len());
        for input in &ordered {
            context.remaining()?;
            vendor_points.push(PointStruct {
                id: Some(canonical_vendor_point_id(input.identity.point_id())),
                payload: encode_payload(&input.point.payload)?,
                vectors: Some(encode_named_vectors(&input.point)?),
            });
        }
        context.remaining()?;
        self.authorize_dispatch()?;
        let acknowledged = tokio::time::timeout(
            context.remaining()?,
            self.client.upsert_points(UpsertPoints {
                collection_name: admission.collection_name.clone(),
                wait: Some(true),
                ordering: Some(super::strong_ordering()),
                points: vendor_points,
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?
        .map_err(super::map_mutation_error)?;
        if !acknowledged
            .result
            .as_ref()
            .is_some_and(|result| super::update_completed(result.status))
        {
            return Err(BridgeError::MutationOutcomeUnknown);
        }
        context.remaining_after_dispatch()?;
        self.authorize_dispatch()
            .map_err(|_| BridgeError::MutationOutcomeUnknown)?;
        let actual = fetch_full_readback(self, admission, &ordered, context)
            .await
            .map_err(|_| BridgeError::MutationOutcomeUnknown)?;
        if !actual.missing_ids.is_empty()
            || actual.points.iter().any(|actual_point| {
                expected_by_id
                    .get(&actual_point.address)
                    .is_none_or(|expected| {
                        !point_readback_equal_exact(&actual_point.point, &expected.point)
                    })
            })
        {
            return Err(BridgeError::MutationOutcomeUnknown);
        }
        context.remaining_after_dispatch()?;
        Ok(actual)
    }

    /// Closes only the exact sorted S11 address set. Same-value close retry is
    /// allowed; another existing upper epoch is immutable. Full point identity
    /// and vectors are read before dispatch and read back afterward.
    pub async fn close_indexed_exact(
        &mut self,
        route: &CollectionRoute,
        expected: &[CanonicalPointInput],
        valid_until_epoch_exclusive: Epoch,
        context: &CanonicalRequestContext,
    ) -> Result<CanonicalMutationReadback, BridgeError> {
        let admission = admission_for(self, route)?;
        let ordered = ordered_inputs(route, admission, expected, self.limits, context)?;
        canonical_epoch_bound(valid_until_epoch_exclusive.get())?;
        for input in &ordered {
            if input
                .point
                .payload
                .valid_until_epoch_exclusive
                .is_some_and(|current| current != valid_until_epoch_exclusive)
            {
                return Err(BridgeError::ExactReadbackMismatch);
            }
        }

        let preflight = fetch_full_readback(self, admission, &ordered, context).await?;
        if !preflight.missing_ids.is_empty() {
            return Err(BridgeError::PointNotFound);
        }
        let expected_by_id: BTreeMap<_, _> = ordered
            .iter()
            .map(|input| (input.identity.point_id(), *input))
            .collect();
        for existing in &preflight.points {
            context.remaining()?;
            let expected = expected_by_id
                .get(&existing.address)
                .ok_or(BridgeError::UnexpectedPoint)?;
            validate_close_epoch(&existing.point.payload, valid_until_epoch_exclusive)?;
            if !point_readback_equal_for_close(&existing.point, &expected.point) {
                return Err(BridgeError::ExactReadbackMismatch);
            }
        }

        let mut closed_inputs = Vec::with_capacity(ordered.len());
        for input in &ordered {
            context.remaining()?;
            let mut closed = (*input).clone();
            closed.point.payload.valid_until_epoch_exclusive = Some(valid_until_epoch_exclusive);
            closed_inputs.push(closed);
        }
        let closed_ordered =
            ordered_inputs(route, admission, &closed_inputs, self.limits, context)?;

        let mut payload = HashMap::new();
        payload.insert(
            PayloadField::ValidUntilEpochExclusive.as_str().to_owned(),
            super::int_value(valid_until_epoch_exclusive.get()),
        );
        context.remaining()?;
        self.authorize_dispatch()?;
        let acknowledged = tokio::time::timeout(
            context.remaining()?,
            self.client.set_payload(SetPayloadPoints {
                collection_name: admission.collection_name.clone(),
                wait: Some(true),
                payload,
                points_selector: Some(PointsSelector {
                    points_selector_one_of: Some(points_selector::PointsSelectorOneOf::Points(
                        PointsIdsList {
                            ids: closed_ordered
                                .iter()
                                .map(|input| canonical_vendor_point_id(input.identity.point_id()))
                                .collect(),
                        },
                    )),
                }),
                ordering: Some(super::strong_ordering()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?
        .map_err(super::map_mutation_error)?;
        if !acknowledged
            .result
            .as_ref()
            .is_some_and(|result| super::update_completed(result.status))
        {
            return Err(BridgeError::MutationOutcomeUnknown);
        }
        context.remaining_after_dispatch()?;
        self.authorize_dispatch()
            .map_err(|_| BridgeError::MutationOutcomeUnknown)?;
        let actual = fetch_full_readback(self, admission, &closed_ordered, context)
            .await
            .map_err(|_| BridgeError::MutationOutcomeUnknown)?;
        if !actual.missing_ids.is_empty()
            || actual.points.iter().any(|actual_point| {
                expected_by_id
                    .get(&actual_point.address)
                    .is_none_or(|input| {
                        let mut closed = input.point.clone();
                        closed.payload.valid_until_epoch_exclusive =
                            Some(valid_until_epoch_exclusive);
                        !point_readback_equal_exact(&actual_point.point, &closed)
                    })
            })
        {
            return Err(BridgeError::MutationOutcomeUnknown);
        }
        context.remaining_after_dispatch()?;
        Ok(actual)
    }

    /// Deletes only exact planned S11 IDs after full hidden-point preflight and
    /// reports success only when exact point-ID readback proves absence.
    pub async fn delete_indexed_exact(
        &mut self,
        route: &CollectionRoute,
        expected: &[CanonicalPointInput],
        context: &CanonicalRequestContext,
    ) -> Result<CanonicalDeleteReadback, BridgeError> {
        let admission = admission_for(self, route)?;
        let ordered = ordered_inputs(route, admission, expected, self.limits, context)?;
        let preflight = fetch_full_readback(self, admission, &ordered, context).await?;
        let expected_by_id: BTreeMap<_, _> = ordered
            .iter()
            .map(|input| (input.identity.point_id(), *input))
            .collect();
        for existing in &preflight.points {
            context.remaining()?;
            let expected = expected_by_id
                .get(&existing.address)
                .ok_or(BridgeError::UnexpectedPoint)?;
            if !point_readback_equal_exact(&existing.point, &expected.point) {
                return Err(BridgeError::ExactReadbackMismatch);
            }
        }

        let ids = ordered
            .iter()
            .map(|input| input.identity.point_id())
            .collect::<Vec<_>>();
        context.remaining()?;
        self.authorize_dispatch()?;
        let acknowledged = tokio::time::timeout(
            context.remaining()?,
            self.client.delete_points(DeletePoints {
                collection_name: admission.collection_name.clone(),
                wait: Some(true),
                points: Some(PointsSelector {
                    points_selector_one_of: Some(points_selector::PointsSelectorOneOf::Points(
                        PointsIdsList {
                            ids: ids.iter().copied().map(canonical_vendor_point_id).collect(),
                        },
                    )),
                }),
                ordering: Some(super::strong_ordering()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?
        .map_err(super::map_mutation_error)?;
        if !acknowledged
            .result
            .as_ref()
            .is_some_and(|result| super::update_completed(result.status))
        {
            return Err(BridgeError::MutationOutcomeUnknown);
        }
        context.remaining_after_dispatch()?;
        self.authorize_dispatch()
            .map_err(|_| BridgeError::MutationOutcomeUnknown)?;
        let present = tokio::time::timeout(
            context.remaining_after_dispatch()?,
            self.client.get_points(GetPoints {
                collection_name: admission.collection_name.clone(),
                ids: ids.iter().copied().map(canonical_vendor_point_id).collect(),
                with_payload: Some(false.into()),
                with_vectors: Some(false.into()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?;
        context.remaining_after_dispatch()?;
        if present.result.len() > ids.len() || !present.result.is_empty() {
            return Err(BridgeError::MutationOutcomeUnknown);
        }
        Ok(CanonicalDeleteReadback { absent_ids: ids })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use search_contracts::indexed::{NamedVectorRequirement, VectorName};
    use search_contracts::{
        AccessPartitionId, BoundedName, CollectionGenerationId, InstallationIncarnationId,
        Modality, ProjectionMembershipId, ProjectionProfileSetId, ScoringDocumentId,
        ScoringPartitionId, SourceId, SourceRevisionId, UnitId, UnitKind,
    };
    use search_point_identity::s11::{
        PointIdentityLimits, PointRole, ProjectionPointKey, derive_point_identity,
    };

    macro_rules! fixture_id {
        ($kind:ty, $number:literal) => {{
            let text = format!("00000000-0000-4000-8000-{number:012}", number = $number);
            <$kind>::parse(&text).expect("fixture ID")
        }};
    }

    fn sample_input() -> (IndexedCollectionSchema, CanonicalPointInput) {
        let installation = fixture_id!(InstallationIncarnationId, 1);
        let generation = fixture_id!(CollectionGenerationId, 2);
        let membership = fixture_id!(ProjectionMembershipId, 3);
        let representation = fixture_id!(search_contracts::RepresentationId, 4);
        let unit = fixture_id!(UnitId, 5);
        let profile = ProjectionProfileSetId::new("lex_code_v1").expect("profile");
        let identity = derive_point_identity(
            &ProjectionPointKey {
                schema_version: search_point_identity::s11::POINT_KEY_SCHEMA_VERSION,
                installation_incarnation_id: installation,
                collection_generation_id: generation,
                projection_membership_id: membership,
                representation_id: representation,
                unit_id: unit,
                projection_profile_set_id: profile.clone(),
                point_role: PointRole::Unit,
            },
            PointIdentityLimits::default(),
        )
        .expect("S11 identity");
        let vector_name = VectorName::new("lex_code_v1").expect("vector name");
        let schema = IndexedCollectionSchema::initial(
            installation,
            generation,
            profile.clone(),
            &[NamedVectorRequirement::new(
                vector_name.clone(),
                VectorRequirement::Sparse {
                    index_ceiling: 32,
                    idf_enabled: true,
                },
            )
            .expect("vector requirement")],
        )
        .expect("schema");
        let payload = search_contracts::indexed::PointPayload::new(
            installation,
            generation,
            membership,
            fixture_id!(AccessPartitionId, 6),
            fixture_id!(ScoringPartitionId, 7),
            fixture_id!(SourceId, 8),
            fixture_id!(SourceRevisionId, 9),
            representation,
            unit,
            identity.full_digest(),
            fixture_id!(ScoringDocumentId, 10),
            profile,
            UnitKind::Symbol,
            Modality::Code,
            BoundedName::new("rust").expect("language"),
            None,
            None,
            None,
            Epoch::new(1).expect("valid from"),
            None,
        );
        let input = CanonicalPointInput {
            identity,
            point: PointReadback {
                payload,
                vectors: vec![NamedVectorValue {
                    name: vector_name,
                    value: VectorValue::Sparse {
                        indices: vec![1, 5],
                        values: vec![1.0, 0.25],
                    },
                }],
            },
        };
        (schema, input)
    }

    #[test]
    fn s11_readback_rejects_changed_identity_coordinate_with_stale_digest() {
        let (_, input) = sample_input();
        let mut changed = input.point.payload.clone();
        changed.unit_id = fixture_id!(UnitId, 11);
        assert_eq!(
            verify_observed_identity(&input.identity, input.identity.point_id(), &changed),
            Err(BridgeError::PointIdCollision)
        );

        let mut changed = input.point.payload.clone();
        changed.representation_id = fixture_id!(search_contracts::RepresentationId, 12);
        assert_eq!(
            verify_observed_identity(&input.identity, input.identity.point_id(), &changed),
            Err(BridgeError::PointIdCollision)
        );

        let mut changed = input.point.payload.clone();
        changed.point_identity_digest_256 =
            search_contracts::Blake3Digest32::from_bytes([0xCC; 32]);
        assert_eq!(
            verify_observed_identity(&input.identity, input.identity.point_id(), &changed),
            Err(BridgeError::PointIdCollision)
        );

        let mut other_key = input.identity.key().clone();
        other_key.unit_id = fixture_id!(UnitId, 13);
        let other_identity = derive_point_identity(&other_key, PointIdentityLimits::default())
            .expect("other S11 identity");
        assert_eq!(
            verify_observed_identity(
                &input.identity,
                other_identity.point_id(),
                &input.point.payload
            ),
            Err(BridgeError::PointIdCollision)
        );
    }

    #[test]
    fn close_epoch_allows_same_value_retry_and_refuses_replacement() {
        let (_, input) = sample_input();
        let requested = Epoch::new(4).expect("close epoch");
        assert_eq!(
            validate_close_epoch(&input.point.payload, requested),
            Ok(())
        );

        let mut existing = input.point.payload.clone();
        existing.valid_until_epoch_exclusive = Some(requested);
        assert_eq!(validate_close_epoch(&existing, requested), Ok(()));
        existing.valid_until_epoch_exclusive = Some(Epoch::new(5).expect("other close"));
        assert_eq!(
            validate_close_epoch(&existing, requested),
            Err(BridgeError::ExactReadbackMismatch)
        );
    }

    #[test]
    fn named_vector_readback_comparison_is_exact_by_float_bits() {
        assert!(!vector_equal_exact(
            &VectorValue::Dense(vec![-0.0]),
            &VectorValue::Dense(vec![0.0])
        ));
        let (_, input) = sample_input();
        let mut changed = input.point.clone();
        changed.vectors[0].value = VectorValue::Sparse {
            indices: vec![1, 5],
            values: vec![1.0, 0.5],
        };
        assert!(!point_readback_equal_exact(&input.point, &changed));
    }
}
