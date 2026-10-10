use super::*;
use search_materializer::api::{MaterializationError, SourceKind};

#[test]
fn real_code_materialization_builds_source_backed_occurrences() {
    let mut descriptor = descriptor();
    descriptor.representation_kind = V3RepresentationKind::Code;
    let profile = validate_v3_unitizer_profile(&descriptor).unwrap();
    let binding = binding();
    let code = "fn main() {}\n";
    let (product, request, materializer) = fixture::materialize_product_with_kind(
        code.as_bytes(),
        &binding.source_id,
        1,
        SourceKind::Code,
    )
    .unwrap();
    let cancel = AtomicBool::new(false);
    let budget = budget(&cancel);
    let input = prepare_unit_set_input(
        binding.clone(),
        &product,
        &request,
        &materializer,
        &profile,
        &budget,
    )
    .unwrap();
    let set = build_unit_manifest(&input, &profile, &budget).unwrap();
    assert_eq!(
        set.manifest().emitted_bytes(),
        u64::try_from(code.len()).unwrap()
    );
    assert_eq!(set.manifest().omitted_bytes(), 0);
    assert_eq!(set.manifest().provenance().binding(), &binding);
    assert_eq!(
        set.manifest().provenance().input_digest(),
        product.input_digest()
    );
    assert_eq!(
        verify_unit_manifest(set.manifest(), &input, &profile, &budget).unwrap(),
        set
    );
}

#[test]
fn representation_kind_and_source_binding_cannot_be_relabelled() {
    let binding = binding();
    for (actual, claimed) in [
        (SourceKind::Text, V3RepresentationKind::Code),
        (SourceKind::Code, V3RepresentationKind::Text),
    ] {
        let (product, request, materializer) =
            fixture::materialize_product_with_kind(b"let x = 1;\n", &binding.source_id, 1, actual)
                .unwrap();
        let mut descriptor = descriptor();
        descriptor.representation_kind = claimed;
        let profile = validate_v3_unitizer_profile(&descriptor).unwrap();
        let cancel = AtomicBool::new(false);
        let budget = budget(&cancel);
        assert!(matches!(
            prepare_unit_set_input(
                binding.clone(),
                &product,
                &request,
                &materializer,
                &profile,
                &budget
            ),
            Err(UnitizationError::UnitManifestIncomplete)
        ));
    }
    let profile = validate_v3_unitizer_profile(&descriptor()).unwrap();
    let (product, request, materializer) =
        fixture::materialize_product(b"one\n", &binding.source_id, 1).unwrap();
    let mut foreign = binding;
    foreign.source_id = SourceId::from_bytes([9; 16]);
    let cancel = AtomicBool::new(false);
    let budget = budget(&cancel);
    assert!(matches!(
        prepare_unit_set_input(
            foreign,
            &product,
            &request,
            &materializer,
            &profile,
            &budget
        ),
        Err(UnitizationError::UnitManifestIncomplete)
    ));
}

#[test]
fn recorded_bom_loss_and_empty_owner_request_are_refused() {
    let binding = binding();
    let profile = validate_v3_unitizer_profile(&descriptor()).unwrap();
    let (product, request, materializer) =
        fixture::materialize_product(b"\xef\xbb\xbfline one\n", &binding.source_id, 1).unwrap();
    assert!(!product.maps().loss_map().records().is_empty());
    let cancel = AtomicBool::new(false);
    let budget = budget(&cancel);
    assert!(matches!(
        prepare_unit_set_input(
            binding.clone(),
            &product,
            &request,
            &materializer,
            &profile,
            &budget
        ),
        Err(UnitizationError::UnitManifestIncomplete)
    ));
    assert!(matches!(
        fixture::materialize_product(b"", &binding.source_id, 1),
        Err(MaterializationError::RequestInvalid)
    ));
}
