use xtask::qualified_profile_id::{
    MAX_LOCAL_OPAQUE_ID_BYTES, MAX_NAMESPACE_BYTES, MAX_QUALIFIED_OPAQUE_ID_BYTES,
    QualifiedOpaqueId, QualifiedOpaqueIdError, VALIDATION_SCOPE,
};

#[test]
fn accepts_all_grammar_character_classes_and_preserves_case() {
    let input = "az09-bc19/AaZz09._-";
    let parsed = QualifiedOpaqueId::parse(input).expect("valid qualified identifier");

    assert_eq!(parsed.value(), input);
    assert_eq!(parsed.namespace(), "az09-bc19");
    assert_eq!(parsed.local(), "AaZz09._-");
    assert_eq!(parsed.value().as_ptr(), input.as_ptr());
    assert_eq!(parsed.namespace().as_ptr(), input.as_ptr());
    assert_eq!(
        parsed.local().as_ptr(),
        input[parsed.namespace().len() + 1..].as_ptr()
    );
    assert_eq!(VALIDATION_SCOPE, "NON_AUTHORITATIVE");
}

#[test]
fn accepts_independent_component_limits_and_the_225_byte_total() {
    let namespace = "a".repeat(MAX_NAMESPACE_BYTES);
    let local = format!("A{}", "_".repeat(MAX_LOCAL_OPAQUE_ID_BYTES - 1));
    let input = format!("{namespace}/{local}");

    assert_eq!(input.len(), MAX_QUALIFIED_OPAQUE_ID_BYTES);
    let parsed = QualifiedOpaqueId::parse(&input).expect("maximum valid identifier");
    assert_eq!(parsed.namespace().len(), MAX_NAMESPACE_BYTES);
    assert_eq!(parsed.local().len(), MAX_LOCAL_OPAQUE_ID_BYTES);
}

#[test]
fn rejects_each_component_over_its_own_limit() {
    let namespace = format!("{}/x", "a".repeat(MAX_NAMESPACE_BYTES + 1));
    assert_eq!(
        QualifiedOpaqueId::parse(&namespace),
        Err(QualifiedOpaqueIdError::NamespaceTooLong)
    );

    let local = format!("a/A{}", "_".repeat(MAX_LOCAL_OPAQUE_ID_BYTES));
    assert_eq!(
        QualifiedOpaqueId::parse(&local),
        Err(QualifiedOpaqueIdError::LocalIdTooLong)
    );
}

#[test]
fn rejects_total_length_over_limit_before_syntax_scanning() {
    let input = format!("!{}/x", "a".repeat(MAX_QUALIFIED_OPAQUE_ID_BYTES));
    assert_eq!(
        QualifiedOpaqueId::parse(&input),
        Err(QualifiedOpaqueIdError::ValueTooLong)
    );
}

#[test]
fn rejects_malformed_syntax_with_stable_bounded_errors() {
    let cases = [
        ("", QualifiedOpaqueIdError::MissingSeparator),
        ("alpha", QualifiedOpaqueIdError::MissingSeparator),
        ("/x", QualifiedOpaqueIdError::EmptyNamespace),
        ("alpha/", QualifiedOpaqueIdError::EmptyLocalId),
        ("alpha/x/y", QualifiedOpaqueIdError::MultipleSeparators),
        ("Alpha/x", QualifiedOpaqueIdError::InvalidNamespace),
        ("a_b/x", QualifiedOpaqueIdError::InvalidNamespace),
        ("-alpha/x", QualifiedOpaqueIdError::InvalidNamespace),
        ("alpha-/x", QualifiedOpaqueIdError::InvalidNamespace),
        ("a--b/x", QualifiedOpaqueIdError::InvalidNamespace),
        ("acme%2Fcore/id", QualifiedOpaqueIdError::InvalidNamespace),
        ("é/id", QualifiedOpaqueIdError::InvalidNamespace),
        ("alpha/_x", QualifiedOpaqueIdError::InvalidLocalId),
        ("alpha/.x", QualifiedOpaqueIdError::InvalidLocalId),
        ("alpha/-x", QualifiedOpaqueIdError::InvalidLocalId),
        ("alpha/id%2Fextra", QualifiedOpaqueIdError::InvalidLocalId),
        ("alpha/id\\extra", QualifiedOpaqueIdError::InvalidLocalId),
        ("alpha/é", QualifiedOpaqueIdError::InvalidLocalId),
        ("alpha/id\0", QualifiedOpaqueIdError::InvalidLocalId),
        ("alpha/id\tx", QualifiedOpaqueIdError::InvalidLocalId),
        ("alpha/id\u{007f}", QualifiedOpaqueIdError::InvalidLocalId),
        ("alpha∕id", QualifiedOpaqueIdError::MissingSeparator),
        ("alpha /id", QualifiedOpaqueIdError::InvalidNamespace),
    ];

    for (input, expected) in cases {
        let error = QualifiedOpaqueId::parse(input).expect_err(input);
        assert_eq!(error, expected, "input: {input:?}");
        assert_eq!(error.to_string(), error.code());
        assert!(error.code().len() <= 32);
    }
}

#[test]
fn preserves_the_exact_borrowed_full_value_and_components() {
    let input = String::from("acme-core/Spec.V1_ABC-9");
    let parsed = QualifiedOpaqueId::parse(&input).expect("valid qualified identifier");

    assert_eq!(parsed.value().as_bytes(), input.as_bytes());
    assert_eq!(parsed.value().as_ptr(), input.as_ptr());
    assert_eq!(parsed.namespace(), "acme-core");
    assert_eq!(parsed.local(), "Spec.V1_ABC-9");
}
