use super::support::*;

#[test]
fn collection_names_reject_non_qdrant_chars_without_network() {
    assert!(validate_collection_name("t24_dataplane_001").is_ok());
    assert!(validate_collection_name("a").is_ok());
    assert!(validate_collection_name("t24-parity_09.Z").is_ok());
    for bad in ["", "has space", "has/slash", "has:colon", "what?"] {
        assert!(
            validate_collection_name(bad).is_err(),
            "must reject without network: {bad:?}"
        );
    }
    let too_long = "a".repeat(200);
    assert!(
        validate_collection_name(&too_long).is_err(),
        "length bound enforced"
    );
}
