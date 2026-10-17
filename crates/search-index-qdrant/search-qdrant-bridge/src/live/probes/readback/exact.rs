//! Exact count/readback, missing-ID and exact-delete qualification probe.

use std::collections::HashMap;

use qdrant_client::qdrant::{
    CountPoints, DeletePoints, GetPoints, PointId, PointStruct, PointsIdsList, PointsSelector,
    RetrievedPoint, Vector, VectorOutput, Vectors, VectorsOutput, point_id, points_selector,
    vector as qdrant_vector, vector_output as qdrant_vector_output, vectors as qdrant_vectors,
    vectors_output as qdrant_vectors_output,
};

use super::super::super::LiveError;
use super::super::super::fixtures::{
    UUID_POINT, VECTOR_CODE, VECTOR_TEXT, base_filter, num_point_id, probe_fixture_points,
    snapshot_id, strong_ordering, update_completed,
};
use super::super::super::suite::Suite;

pub(in crate::live) async fn probe_count_and_readback(
    suite: &mut Suite<'_>,
) -> Result<(), LiveError> {
    let count = suite
        .client
        .count(CountPoints {
            collection_name: suite.client.collection_name().to_owned(),
            filter: Some(base_filter()?),
            exact: Some(true),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let readback = suite
        .client
        .get_points(GetPoints {
            collection_name: suite.client.collection_name().to_owned(),
            ids: vec![
                num_point_id(1),
                num_point_id(2),
                num_point_id(9),
                PointId {
                    point_id_options: Some(point_id::PointIdOptions::Uuid(UUID_POINT.to_owned())),
                },
            ],
            with_payload: Some(true.into()),
            with_vectors: Some(true.into()),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let expected_by_id: HashMap<String, PointStruct> = probe_fixture_points()
        .into_iter()
        .map(|point| (snapshot_id(point.id.as_ref()), point))
        .collect();
    let mut found: Vec<String> = readback
        .result
        .iter()
        .map(|point| snapshot_id(point.id.as_ref()))
        .collect();
    found.sort();
    let full_fixture_ok = readback.result.iter().all(|point| {
        expected_by_id
            .get(&snapshot_id(point.id.as_ref()))
            .is_some_and(|expected| fixture_point_readback_matches(point, expected))
    });
    let unknown = suite
        .client
        .get_points(GetPoints {
            collection_name: suite.client.collection_name().to_owned(),
            ids: vec![num_point_id(777)],
            with_payload: Some(false.into()),
            with_vectors: Some(false.into()),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let passed = count.result.as_ref().map(|result| result.count) == Some(4)
        && found == ["1", "2", "550e8400-e29b-41d4-a716-446655440000", "9"]
        && full_fixture_ok
        && unknown.result.is_empty();
    suite.record(
        "exact_count_and_readback",
        passed,
        format!(
            "count={:?} ids={found:?} full_fixture={full_fixture_ok} unknown_empty={}",
            count.result.as_ref().map(|result| result.count),
            unknown.result.is_empty()
        ),
    );

    let deleted = suite
        .client
        .delete_points(DeletePoints {
            collection_name: suite.client.collection_name().to_owned(),
            wait: Some(true),
            points: Some(PointsSelector {
                points_selector_one_of: Some(points_selector::PointsSelectorOneOf::Points(
                    PointsIdsList {
                        ids: vec![num_point_id(2)],
                    },
                )),
            }),
            ordering: Some(strong_ordering()),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    if !deleted
        .result
        .as_ref()
        .is_some_and(|result| update_completed(result.status))
    {
        return Err(LiveError::ProbeFailed {
            probe: "exact_count_and_readback",
        });
    }
    let recount = suite
        .client
        .count(CountPoints {
            collection_name: suite.client.collection_name().to_owned(),
            filter: Some(base_filter()?),
            exact: Some(true),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    suite.log.push(format!(
        "RECLAIM point=2 post_count={:?}",
        recount.result.as_ref().map(|result| result.count)
    ));
    if recount.result.as_ref().map(|result| result.count) != Some(3) {
        return Err(LiveError::ProbeFailed {
            probe: "exact_count_and_readback",
        });
    }
    Ok(())
}

fn fixture_point_readback_matches(actual: &RetrievedPoint, expected: &PointStruct) -> bool {
    actual.id == expected.id
        && actual.payload == expected.payload
        && expected
            .vectors
            .as_ref()
            .zip(actual.vectors.as_ref())
            .is_some_and(|(expected, actual)| named_sparse_vectors_match(expected, actual))
}

fn named_sparse_vectors_match(expected: &Vectors, actual: &VectorsOutput) -> bool {
    let Some(qdrant_vectors::VectorsOptions::Vectors(expected)) = expected.vectors_options.as_ref()
    else {
        return false;
    };
    let Some(qdrant_vectors_output::VectorsOptions::Vectors(actual)) =
        actual.vectors_options.as_ref()
    else {
        return false;
    };
    expected.vectors.len() == 2
        && actual.vectors.len() == expected.vectors.len()
        && expected.vectors.contains_key(VECTOR_CODE)
        && expected.vectors.contains_key(VECTOR_TEXT)
        && expected.vectors.iter().all(|(name, expected)| {
            actual
                .vectors
                .get(name)
                .is_some_and(|actual| sparse_vector_bits_match(expected, actual))
        })
}

fn sparse_vector_bits_match(expected: &Vector, actual: &VectorOutput) -> bool {
    let Some(qdrant_vector::Vector::Sparse(expected)) = expected.vector.as_ref() else {
        return false;
    };
    let qdrant_vector_output::Vector::Sparse(actual) = actual.clone().into_vector() else {
        return false;
    };
    expected.indices == actual.indices
        && expected.values.len() == actual.values.len()
        && expected
            .values
            .iter()
            .zip(&actual.values)
            .all(|(expected, actual)| expected.to_bits() == actual.to_bits())
}

#[cfg(test)]
mod tests {
    use super::*;
    use qdrant_client::qdrant::{NamedVectorsOutput, SparseVector};

    fn sparse_output(indices: Vec<u32>, values: Vec<f32>) -> VectorOutput {
        VectorOutput {
            vector: Some(qdrant_vector_output::Vector::Sparse(SparseVector {
                indices,
                values,
            })),
            ..Default::default()
        }
    }

    fn named_output(code: VectorOutput, text: Option<VectorOutput>) -> VectorsOutput {
        let mut vectors = HashMap::new();
        vectors.insert(VECTOR_CODE.to_owned(), code);
        if let Some(text) = text {
            vectors.insert(VECTOR_TEXT.to_owned(), text);
        }
        VectorsOutput {
            vectors_options: Some(qdrant_vectors_output::VectorsOptions::Vectors(
                NamedVectorsOutput { vectors },
            )),
        }
    }

    #[test]
    fn complete_fixture_readback_rejects_missing_payload_or_named_vector() {
        let expected = probe_fixture_points()
            .into_iter()
            .next()
            .expect("first fixture point");
        let mut actual = RetrievedPoint {
            id: expected.id.clone(),
            payload: expected.payload.clone(),
            vectors: Some(named_output(
                sparse_output(vec![0, 1], vec![1.0, 1.0]),
                Some(sparse_output(vec![100], vec![1.0])),
            )),
            ..Default::default()
        };
        assert!(fixture_point_readback_matches(&actual, &expected));

        actual
            .payload
            .remove(crate::live::fixtures::FIELD_UNINDEXED);
        assert!(!fixture_point_readback_matches(&actual, &expected));

        actual.payload = expected.payload.clone();
        actual.vectors = Some(named_output(
            sparse_output(vec![0, 1], vec![1.0, 1.0]),
            None,
        ));
        assert!(!fixture_point_readback_matches(&actual, &expected));
    }
    #[test]
    fn exact_fixture_vectors_require_names_indices_and_float_bits() {
        let expected =
            crate::live::fixtures::sparse_named(vec![(1, 0.0), (3, 1.0)], vec![(101, 2.0)]);
        let exact = named_output(
            sparse_output(vec![1, 3], vec![0.0, 1.0]),
            Some(sparse_output(vec![101], vec![2.0])),
        );
        assert!(named_sparse_vectors_match(&expected, &exact));

        let missing_text = named_output(sparse_output(vec![1, 3], vec![0.0, 1.0]), None);
        assert!(!named_sparse_vectors_match(&expected, &missing_text));

        let wrong_indices = named_output(
            sparse_output(vec![1, 4], vec![0.0, 1.0]),
            Some(sparse_output(vec![101], vec![2.0])),
        );
        assert!(!named_sparse_vectors_match(&expected, &wrong_indices));

        let wrong_float_bits = named_output(
            sparse_output(vec![1, 3], vec![-0.0, 1.0]),
            Some(sparse_output(vec![101], vec![2.0])),
        );
        assert!(!named_sparse_vectors_match(&expected, &wrong_float_bits));
    }
}
