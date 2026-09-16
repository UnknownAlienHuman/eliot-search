use search_contracts::{
    AccessPartitionId, BoundedSet, CorpusId, CorpusOrPortfolioId, DisclosureCeiling, Modality,
    ProfileId, RecipeIdV1, ReferencePortfolioId, SensitivityClass, SourceMembershipId,
};

use crate::error::ProtocolError;

use super::*;

fn set<T: Ord, const LIMIT: usize>(
    values: impl IntoIterator<Item = T>,
) -> BoundedSet<T, LIMIT> {
    BoundedSet::from_items(values).expect("bounded set")
}

fn request() -> StandaloneGrantRequestV1 {
    StandaloneGrantRequestV1 {
        expected_binding_generation: 4,
        expected_policy_generation: 7,
        requested_membership_ids: set([
            SourceMembershipId::from_bytes([0x10; 16]),
            SourceMembershipId::from_bytes([0x11; 16]),
        ]),
        requested_corpus_or_portfolio_ids: set([
            CorpusOrPortfolioId::Corpus(CorpusId::from_bytes([0x20; 16])),
            CorpusOrPortfolioId::Portfolio(ReferencePortfolioId::from_bytes([0x21; 16])),
        ]),
        requested_access_partitions: set([AccessPartitionId::from_bytes([0x30; 16])]),
        requested_modalities: set([Modality::Code, Modality::Text]),
        requested_recipe_families: set([RecipeIdV1::Locate, RecipeIdV1::FindText]),
        requested_budget_class: ProfileId::new("interactive.v1").expect("profile"),
        requested_sensitivity_ceiling: SensitivityClass::Project,
        requested_disclosure_ceiling: DisclosureCeiling::LocalOnly,
        requested_source_read_permission: true,
        requested_exact_scan_permission: false,
        requested_ttl_ms: 30_000,
    }
}

#[test]
fn canonical_body_round_trips_exactly() {
    let request = request();
    let encoded = encode_standalone_grant_request(&request).expect("encode");
    assert_eq!(
        decode_standalone_grant_request(&encoded).expect("decode"),
        request
    );
    assert!(encoded.starts_with(b"{\"v\":1,\"binding_generation\":4,"));
    assert!(encoded.ends_with(b"\"ttl_ms\":30000}"));
}

#[test]
fn alternate_order_case_duplicates_and_trailing_bytes_fail() {
    let canonical = encode_standalone_grant_request(&request()).expect("encode");

    let mut upper = canonical.clone();
    let budget_hex = b"696e7465726163746976652e7631";
    let budget_start = upper
        .windows(budget_hex.len())
        .position(|window| window == budget_hex)
        .expect("budget hex");
    let relative = budget_hex
        .iter()
        .position(|byte| matches!(*byte, b'a'..=b'f'))
        .expect("hex letter");
    upper[budget_start + relative] = upper[budget_start + relative].to_ascii_uppercase();
    assert_eq!(
        decode_standalone_grant_request(&upper),
        Err(ProtocolError::InvalidBody)
    );

    let duplicate = canonical
        .windows(68)
        .position(|window| window.starts_with(b"\"10101010101010101010101010101010\","))
        .expect("membership entry");
    let mut duplicated = canonical.clone();
    duplicated.splice(
        duplicate..duplicate,
        b"\"10101010101010101010101010101010\",".iter().copied(),
    );
    assert_eq!(
        decode_standalone_grant_request(&duplicated),
        Err(ProtocolError::InvalidBody)
    );

    let mut trailing = canonical;
    trailing.push(b' ');
    assert_eq!(
        decode_standalone_grant_request(&trailing),
        Err(ProtocolError::InvalidBody)
    );
}

#[test]
fn zero_empty_and_widened_permission_shapes_fail_closed() {
    let mut invalid = request();
    invalid.expected_policy_generation = 0;
    assert_eq!(invalid.validate(), Err(ProtocolError::InvalidBody));

    let mut invalid = request();
    invalid.requested_membership_ids = BoundedSet::empty();
    assert_eq!(invalid.validate(), Err(ProtocolError::InvalidBody));

    let mut invalid = request();
    invalid.requested_source_read_permission = false;
    invalid.requested_exact_scan_permission = true;
    assert_eq!(invalid.validate(), Err(ProtocolError::InvalidBody));
}

#[test]
fn oversized_body_fails_before_parsing() {
    let body = vec![b'x'; MAX_STANDALONE_GRANT_REQUEST_BYTES + 1];
    assert_eq!(
        decode_standalone_grant_request(&body),
        Err(ProtocolError::FrameTooLarge)
    );
}
