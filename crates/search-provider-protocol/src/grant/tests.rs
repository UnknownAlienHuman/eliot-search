use search_contracts::{
    AccessPartitionId, BoundedSet, CorpusId, CorpusOrPortfolioId, DisclosureCeiling, Modality,
    ProfileId, RecipeIdV1, ReferencePortfolioId, SearchReadGrantClaims, SensitivityClass,
    SourceMembershipId, UtcTimestamp,
};
use search_contracts::evidence::RecipeIdV2;

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

fn evidence_request_v2() -> StandaloneGrantRequestV2 {
    StandaloneGrantRequestV2 {
        expected_binding_generation: 4,
        expected_policy_generation: 7,
        requested_membership_ids: set([SourceMembershipId::from_bytes([0x10; 16])]),
        requested_corpus_or_portfolio_ids: BoundedSet::empty(),
        requested_access_partitions: set([AccessPartitionId::from_bytes([0x30; 16])]),
        requested_modalities: set([Modality::Text]),
        requested_evidence_recipes: set([
            RecipeIdV2::RetrieveEvidence,
            RecipeIdV2::OrientScope,
        ]),
        requested_budget_class: ProfileId::new("interactive.v1").expect("profile"),
        requested_sensitivity_ceiling: SensitivityClass::Project,
        requested_disclosure_ceiling: DisclosureCeiling::LocalOnly,
        requested_source_read_permission: true,
        requested_exact_scan_permission: false,
        requested_ttl_ms: 30_000,
    }
}

fn evidence_claims_v2() -> SearchReadGrantClaimsV2 {
    SearchReadGrantClaimsV2 {
        claims: SearchReadGrantClaims {
            grant_id: search_contracts::GrantId::from_bytes([0x41; 16]),
            installation_id: search_contracts::InstallationId::from_bytes([0x42; 16]),
            installation_incarnation_id:
                search_contracts::InstallationIncarnationId::from_bytes([0x43; 16]),
            binding_id: search_contracts::BindingId::from_bytes([0x44; 16]),
            principal_opaque_id: search_contracts::OpaqueId::new("principal").expect("principal"),
            client_scope_ref: search_contracts::OpaqueRef::new("scope").expect("scope"),
            scope_domain_id: search_contracts::ScopeDomainId::from_bytes([0x45; 16]),
            allowed_membership_ids: set([SourceMembershipId::from_bytes([0x10; 16])]),
            allowed_corpus_or_portfolio_ids: BoundedSet::empty(),
            reference_portfolio_revision: None,
            allowed_access_partitions: set([AccessPartitionId::from_bytes([0x30; 16])]),
            allowed_modalities: set([Modality::Text]),
            permitted_recipe_families: BoundedSet::empty(),
            maximum_budget_class: ProfileId::new("interactive.v1").expect("profile"),
            sensitivity_ceiling: SensitivityClass::Project,
            disclosure_ceiling: DisclosureCeiling::LocalOnly,
            source_read_permission: true,
            exact_scan_permission: false,
            issued_boot_id: search_contracts::OpaqueId::new("boot").expect("boot"),
            issued_at: UtcTimestamp::parse("2026-10-16T00:00:00.000000Z").expect("issued"),
            expires_at: UtcTimestamp::parse("2026-10-16T00:00:30.000000Z").expect("expires"),
            nonce: search_contracts::OpaqueId::new("grant-nonce").expect("nonce"),
            revocation_generation: 1,
        },
        permitted_evidence_recipes: set([RecipeIdV2::RetrieveEvidence]),
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

#[test]
fn evidence_grant_v2_is_versioned_and_does_not_widen_v1_decode() {
    let request = evidence_request_v2();
    let encoded = encode_standalone_grant_request_v2(&request).expect("encode v2");
    assert!(encoded.starts_with(b"{\"v\":2,\"binding_generation\":4,"));
    assert!(encoded.windows(18).any(|field| field == b"\"evidence_recipes\""));
    assert_eq!(decode_standalone_grant_request_v2(&encoded).expect("decode"), request);
    assert_eq!(
        decode_standalone_grant_request(&encoded),
        Err(ProtocolError::InvalidVersion)
    );

    let mut unknown = encoded;
    let index = unknown
        .windows(b"evidence_recipes".len())
        .position(|field| field == b"evidence_recipes")
        .expect("field");
    unknown[index + b"evidence_recipes".len() - 1] = b'x';
    assert_eq!(
        decode_standalone_grant_request_v2(&unknown),
        Err(ProtocolError::InvalidBody)
    );
}

#[test]
fn evidence_grant_response_v2_round_trips_claims_with_explicit_recipe_permission() {
    use crate::request::RequestStatus;

    let body = StandaloneGrantResponseBodyV2::Claims(evidence_claims_v2());
    let encoded = encode_standalone_grant_response_body_v2(RequestStatus::Ok, &body)
        .expect("encode response");
    assert!(encoded.starts_with(b"{\"v\":2,\"outcome\":{\"claims_v2\":"));
    assert_eq!(
        decode_standalone_grant_response_body_v2(RequestStatus::Ok, &encoded)
            .expect("decode response"),
        body
    );
    assert_eq!(
        decode_standalone_grant_response_body(RequestStatus::Ok, &encoded),
        Err(ProtocolError::NoCompatibleVersion)
    );
}
