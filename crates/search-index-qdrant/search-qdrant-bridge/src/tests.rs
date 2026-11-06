//! Pure SDK-shape and pre-dispatch boundary cases; no server is started.

use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use prost::Message;
use qdrant_client::qdrant::{UpdateMode, payload_index_params, stemming_algorithm, vectors};
use search_contracts::SourceId;

use crate::{Backend, BackendError, BackendLimits, MAX_REQUEST_BYTES, PointInput};
use crate::{codec, schema};

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(5)
}

fn point(id: u8, text: &str) -> PointInput<'_> {
    PointInput {
        id: [id; 16],
        source_id: SourceId::from_bytes([2; 16]),
        retrieval_text: text,
    }
}

#[test]
fn pure_measurement_is_actual_protobuf_and_upload_is_vectorless_insert_only() {
    let cancel = AtomicBool::new(false);
    let points = [point(1, "real exact unit text")];
    let request = codec::encode_batch("generation_1", &points, deadline(), &cancel).unwrap();
    assert_eq!(
        Backend::batch_request_bytes("generation_1", &points, deadline(), &cancel).unwrap(),
        request.encoded_len()
    );
    assert_eq!(request.wait, Some(true));
    assert_eq!(request.update_mode, Some(i32::from(UpdateMode::InsertOnly)));
    assert!(matches!(
        request.points[0].vectors.as_ref().and_then(|vectors| vectors.vectors_options.as_ref()),
        Some(vectors::VectorsOptions::Vectors(named)) if named.vectors.is_empty()
    ));
    assert_eq!(request.points[0].payload.len(), 2);
    assert_eq!(
        codec::payload_string(&request.points[0].payload, "retrieval_text").unwrap(),
        points[0].retrieval_text
    );
    assert_eq!(
        codec::decode_id(request.points[0].id.as_ref()).unwrap(),
        points[0].id
    );
}

#[test]
fn wire_overhead_is_charged_without_truncating_complete_units() {
    let text = "x".repeat(65_536);
    let points: Vec<_> = (0_u8..16).map(|id| point(id, &text)).collect();
    let cancel = AtomicBool::new(false);
    assert!(
        Backend::batch_request_bytes("g", &points[..15], deadline(), &cancel).unwrap()
            < MAX_REQUEST_BYTES
    );
    assert_eq!(
        Backend::batch_request_bytes("g", &points, deadline(), &cancel),
        Err(BackendError::RequestTooLarge)
    );
    assert_eq!(points[15].retrieval_text.len(), 65_536);
}

#[test]
fn duplicate_ids_count_and_oversized_unit_are_definite_pre_dispatch_refusals() {
    let cancel = AtomicBool::new(false);
    assert_eq!(
        Backend::batch_request_bytes("g", &[point(1, "a"), point(1, "b")], deadline(), &cancel),
        Err(BackendError::InvalidInput)
    );
    let too_many: Vec<_> = (0_u8..129).map(|id| point(id, "x")).collect();
    assert_eq!(
        Backend::batch_request_bytes("g", &too_many, deadline(), &cancel),
        Err(BackendError::RequestTooLarge)
    );
    let text = "x".repeat(65_537);
    assert_eq!(
        Backend::batch_request_bytes("g", &[point(1, &text)], deadline(), &cancel),
        Err(BackendError::RequestTooLarge)
    );
}

#[test]
fn physical_request_contains_complete_explicit_code_bm25_profile() {
    let collection = schema::create_collection("g");
    assert!(collection.vectors_config.is_none());
    assert!(collection.sparse_vectors_config.is_none());
    assert_eq!(collection.shard_number, Some(1));
    let index = schema::create_text_index("g");
    assert_eq!(index.wait, Some(true));
    let Some(payload_index_params::IndexParams::TextIndexParams(params)) =
        index.field_index_params.unwrap().index_params
    else {
        panic!("text parameters absent");
    };
    assert_eq!(params.lowercase, Some(true));
    assert_eq!(params.ascii_folding, Some(false));
    assert_eq!(params.phrase_matching, Some(false));
    assert_eq!(params.enable_hnsw, Some(false));
    assert!(params.scoring.is_some());
    assert!(matches!(
        params.stemmer.unwrap().stemming_params,
        Some(stemming_algorithm::StemmingParams::Disabled(_))
    ));
    assert!(params.stopwords.is_none());
    assert!(params.min_token_len.is_none());
    assert!(params.max_token_len.is_none());
    assert!(params.nested.is_none());
}

#[test]
fn remote_ambiguous_and_credential_urls_are_refused_before_connect() {
    let cancel = AtomicBool::new(false);
    for endpoint in [
        "http://192.0.2.1:6334",
        "http://localhost:6334",
        "http://user:secret@127.0.0.1:6334",
        "http://127.0.0.1:6334/path",
    ] {
        assert_eq!(
            Backend::connect(
                endpoint,
                None,
                BackendLimits::default(),
                deadline(),
                &cancel
            )
            .unwrap_err(),
            BackendError::InvalidEndpoint
        );
    }
}

#[test]
fn cancelled_expired_and_redacted_inputs_keep_their_real_meaning() {
    let points = [point(1, "private_payload_sentinel")];
    assert_eq!(
        Backend::batch_request_bytes("g", &points, deadline(), &AtomicBool::new(true)),
        Err(BackendError::Cancelled)
    );
    assert_eq!(
        Backend::batch_request_bytes("g", &points, Instant::now(), &AtomicBool::new(false)),
        Err(BackendError::Deadline)
    );
    assert!(!format!("{:?}", points[0]).contains("private_payload_sentinel"));
}
