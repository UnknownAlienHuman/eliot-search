use search_contracts::{
    AccessPartitionId, BindingId, BoundedSet, CorpusId, CorpusOrPortfolioId,
    DisclosureCeiling, Modality, ProfileId, RecipeIdV1, RequestId,
    SensitivityClass, SourceMembershipId, MAX_SET_ITEMS,
};
use search_provider_protocol::StandaloneGrantRequestV1;

use super::{GrantAuthorityError, map_protocol_request};

fn set<T: Ord, const LIMIT: usize>(
    values: impl IntoIterator<Item = T>,
) -> BoundedSet<T, LIMIT> {
    BoundedSet::from_items(values).expect("bounded set")
}

fn request() -> StandaloneGrantRequestV1 {
    StandaloneGrantRequestV1 {
        expected_binding_generation: 4,
        expected_policy_generation: 7,
        requested_membership_ids: set([SourceMembershipId::from_bytes([10; 16])]),
        requested_corpus_or_portfolio_ids: set([CorpusOrPortfolioId::Corpus(
            CorpusId::from_bytes([20; 16]),
        )]),
        requested_access_partitions: set([AccessPartitionId::from_bytes([30; 16])]),
        requested_modalities: set([Modality::Code]),
        requested_recipe_families: set([RecipeIdV1::Locate]),
        requested_budget_class: ProfileId::new("interactive").expect("profile"),
        requested_sensitivity_ceiling: SensitivityClass::Project,
        requested_disclosure_ceiling: DisclosureCeiling::LocalOnly,
        requested_source_read_permission: true,
        requested_exact_scan_permission: false,
        requested_ttl_ms: 30_000,
    }
}

#[test]
fn protocol_body_cannot_choose_binding_or_operation_identity() {
    let binding = BindingId::from_bytes([1; 16]);
    let request_id = RequestId::from_bytes([0x55; 16]);
    let mapped = map_protocol_request(binding, &request_id, request()).expect("mapped request");

    assert_eq!(mapped.binding_id, binding);
    assert_eq!(mapped.operation_id.as_str(), "standalone-grant-v1:55555555555555555555555555555555");
    assert_eq!(mapped.expected_binding_generation, 4);
    assert_eq!(mapped.expected_policy_generation, 7);
    assert_eq!(mapped.requested_ttl_ms, 30_000);
}

#[test]
fn invalid_protocol_shape_is_rejected_before_authority_access() {
    let mut body = request();
    body.expected_policy_generation = 0;
    assert_eq!(
        map_protocol_request(
            BindingId::from_bytes([1; 16]),
            &RequestId::from_bytes([0x55; 16]),
            body,
        ),
        Err(GrantAuthorityError::ProtocolRequestInvalid)
    );
}
