//! Fixed P00 bytes for every existing v1 recipe, decoded and encoded by the actual schema.
use super::wire::{Decoder, Encoder, Schema};
use search_contracts::*;

#[test]
fn all_eleven_v1_bodies_keep_exact_p00_json_and_shared_frame_bytes() {
    let subject = r#"{"normalized_name":{"name":"owner","entity_kinds":[]}}"#;
    let predicate = r#"{"kind":"literal","engine_and_version":"literal-v1","serialized_form":"bmVlZGxl","input_domain":"decoded_text","worst_case_complexity_class":"linear-scan"}"#;
    let portfolio =
        r#"{"portfolio_id":"11111111-1111-1111-1111-111111111111","portfolio_revision":1}"#;
    let view =
        r#"{"retained_revision":{"retained_revision_id":"11111111-1111-1111-1111-111111111111"}}"#;
    let handle = r#"{"handle_id":"11111111-1111-1111-1111-111111111111","handle_revision":1,"handle_class":"durable_source","expires_at":null,"opaque_token":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}"#;
    let goldens = [
        format!(r#"{{"locate@1":{{"subject":{subject},"evidence_roles":[]}}}}"#),
        format!(r#"{{"find_text@1":{{"predicate":{predicate},"case_policy":"exact","context_bytes_before":32,"context_bytes_after":32}}}}"#),
        format!(r#"{{"inspect_entity@1":{{"subject":{subject},"evidence_roles":[],"include_relations":[]}}}}"#),
        format!(r#"{{"compare_implementations@1":{{"subject":{subject},"references":{portfolio},"comparison_axes":[]}}}}"#),
        format!(r#"{{"explore_entity@1":{{"subject":{subject},"relation_kinds":[],"max_depth":1}}}}"#),
        r#"{"corpus_profile@1":{"facets":[]}}"#.to_owned(),
        format!(r#"{{"corpus_delta@1":{{"from_view":{view},"to_view":{view},"dimensions":[]}}}}"#),
        format!(r#"{{"provenance@1":{{"source_handle":{handle},"max_lineage_depth":1}}}}"#),
        format!(r#"{{"compile_exact_scan@1":{{"predicate":{predicate},"completeness_requirements":{{"require_every_denominator_item":true,"require_stable_or_retained_revision":true,"require_current_observation":true,"include_authenticated_unsaved_buffers":false,"fail_on_timeout":true,"fail_on_cancellation":true,"fail_on_scope_drift":true}}}}}}"#),
        r#"{"execute_exact_scan@1":{"plan_ref":{"plan_id":"11111111-1111-1111-1111-111111111111","plan_fingerprint":"1111111111111111111111111111111111111111111111111111111111111111"}}}"#.to_owned(),
        format!(r#"{{"expand_handle@1":{{"handle":{{"source":{handle}}},"expansion":"excerpt","max_bytes":1024}}}}"#),
    ];
    for (expected_recipe, golden) in RecipeIdV1::ALL.into_iter().zip(goldens) {
        let mut decoder = Decoder::new(golden.as_bytes());
        let body = RecipeBodyV1::get(&mut decoder).expect("fixed P00 body");
        decoder.finish().expect("entire golden consumed");
        assert_eq!(body.recipe_id(), expected_recipe);
        let mut encoder = Encoder::new(MAX_FRAME_BYTES - 4);
        body.put(&mut encoder).expect("encode unchanged v1");
        assert_eq!(
            encoder.finish().expect("balanced JSON"),
            golden.as_bytes(),
            "{expected_recipe:?}"
        );
        let payload = JsonFramePayload::new(golden.as_bytes().to_vec()).expect("payload");
        let frame =
            crate::encode_frame(&payload, crate::DEFAULT_PROTOCOL_LIMITS).expect("shared frame");
        let mut expected = u32::try_from(golden.len())
            .expect("length")
            .to_le_bytes()
            .to_vec();
        expected.extend_from_slice(golden.as_bytes());
        assert_eq!(frame.as_slice(), expected);
        assert_eq!(
            crate::decode_frame(&expected, crate::DEFAULT_PROTOCOL_LIMITS).expect("decode"),
            payload
        );
    }
}
