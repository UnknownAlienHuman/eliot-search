//! Shared construction-budget fixtures. AUTHORED_NOT_RUN; integrated #238/#215 qualification.

use search_config::{
    ConfigDocument, ConfigError, ConfigKeyName, ConfigKeyPath, ConfigLimits, ConfigSectionName,
    ConfigSource, ConfigSourceKind, ConfigSourceRef, DocumentValue, LayerOperation, parse_document,
};
use search_contracts::{Blake3Digest32, ProfileId};

fn source(kind: ConfigSourceKind) -> ConfigSource {
    ConfigSource {
        kind,
        source_ref: ConfigSourceRef::new("synthetic", 32).expect("source"),
        source_digest: Blake3Digest32::from_bytes([1; 32]),
    }
}

fn path(section: &str, key: &str) -> ConfigKeyPath {
    ConfigKeyPath::new(
        ConfigSectionName::new(section, 128).expect("section"),
        ConfigKeyName::new(key, 128).expect("key"),
    )
}

fn set(value: DocumentValue) -> LayerOperation {
    LayerOperation::Set(value)
}

fn file(tail: &str, limits: ConfigLimits) -> Result<ConfigDocument, ConfigError> {
    let bytes = format!("schema_version=2\nprofile='direct'\n{tail}");
    parse_document(bytes.as_bytes(), source(ConfigSourceKind::File), limits)
}

fn captured(
    kind: ConfigSourceKind,
    entries: impl IntoIterator<Item = (ConfigKeyPath, LayerOperation)>,
    limits: ConfigLimits,
) -> Result<ConfigDocument, ConfigError> {
    let profile =
        (kind == ConfigSourceKind::File).then(|| ProfileId::new("direct").expect("profile"));
    let schema = if kind == ConfigSourceKind::File { 2 } else { 1 };
    ConfigDocument::from_entries(schema, profile, source(kind), entries, limits)
}

#[test]
fn file_and_captured_layers_share_exact_entries() {
    let parsed = file("[a]\nx=true\ny=-7\nz=['é','']\n", ConfigLimits::W1).expect("file");
    for kind in [
        ConfigSourceKind::File,
        ConfigSourceKind::Environment,
        ConfigSourceKind::Cli,
    ] {
        let built = captured(
            kind,
            [
                (path("a", "x"), set(DocumentValue::Boolean(true))),
                (path("a", "y"), set(DocumentValue::Integer(-7))),
                (
                    path("a", "z"),
                    set(DocumentValue::StringList(vec![
                        "é".to_owned(),
                        String::new(),
                    ])),
                ),
            ],
            ConfigLimits::W1,
        )
        .expect("captured");
        assert_eq!(
            built.entries().collect::<Vec<_>>(),
            parsed.entries().collect::<Vec<_>>()
        );
        assert_eq!(
            built.declared_sections().collect::<Vec<_>>(),
            parsed.declared_sections().collect::<Vec<_>>()
        );
        if kind == ConfigSourceKind::File {
            assert_eq!(built, parsed);
        }
    }
}

#[test]
fn section_limit_includes_empty_file_tables_and_captured_names() {
    let mut limits = ConfigLimits::W1;
    limits.max_sections = 1;
    assert!(file("[a]\n", limits).is_ok());
    assert_eq!(
        file("[a]\n[b]\n", limits),
        Err(ConfigError::CapacityExceeded)
    );
    for kind in [ConfigSourceKind::Environment, ConfigSourceKind::Cli] {
        assert_eq!(
            captured(
                kind,
                [
                    (path("a", "x"), LayerOperation::Reset),
                    (path("b", "x"), LayerOperation::Reset),
                ],
                limits
            ),
            Err(ConfigError::CapacityExceeded)
        );
    }
}

#[test]
fn repeated_section_entries_use_one_section_budget() {
    let mut limits = ConfigLimits::W1;
    limits.max_sections = 1;
    let parsed = file("[a]\nx=1\ny=2\n", limits).expect("one section");
    let built = captured(
        ConfigSourceKind::File,
        [
            (path("a", "x"), set(DocumentValue::Integer(1))),
            (path("a", "y"), set(DocumentValue::Integer(2))),
        ],
        limits,
    )
    .expect("one captured section");
    assert_eq!(parsed, built);
    assert_eq!(built.declared_sections().len(), 1);
}

#[test]
fn fields_are_bounded_per_section_for_every_layer() {
    let mut limits = ConfigLimits::W1;
    limits.max_fields_per_section = 1;
    assert!(file("[a]\nx=1\n[b]\nx=2\n", limits).is_ok());
    assert_eq!(
        file("[a]\nx=1\ny=2\n", limits),
        Err(ConfigError::CapacityExceeded)
    );
    for kind in [
        ConfigSourceKind::File,
        ConfigSourceKind::Environment,
        ConfigSourceKind::Cli,
    ] {
        assert_eq!(
            captured(
                kind,
                [
                    (path("a", "x"), LayerOperation::Reset),
                    (path("a", "y"), LayerOperation::Reset),
                ],
                limits
            ),
            Err(ConfigError::CapacityExceeded)
        );
    }
}

#[test]
fn entry_limit_is_independent_of_section_fields() {
    let mut limits = ConfigLimits::W1;
    limits.max_entries_per_layer = 1;
    assert!(file("[a]\nx=1\n", limits).is_ok());
    assert_eq!(
        file("[a]\nx=1\n[b]\ny=2\n", limits),
        Err(ConfigError::CapacityExceeded)
    );
    assert_eq!(
        captured(
            ConfigSourceKind::Cli,
            [
                (path("a", "x"), LayerOperation::Reset),
                (path("b", "y"), LayerOperation::Reset),
            ],
            limits
        ),
        Err(ConfigError::CapacityExceeded)
    );
}

#[test]
fn identifier_total_counts_profile_table_and_path_once() {
    let mut limits = ConfigLimits::W1;
    limits.max_decoded_identifier_bytes_total = 12; // direct6 + aa2 + path aa2/bb2
    assert!(file("[aa]\nbb=true\n", limits).is_ok());
    assert!(
        captured(
            ConfigSourceKind::File,
            [(path("aa", "bb"), set(DocumentValue::Boolean(true)))],
            limits
        )
        .is_ok()
    );
    limits.max_decoded_identifier_bytes_total = 11;
    assert_eq!(
        file("[aa]\nbb=true\n", limits),
        Err(ConfigError::CapacityExceeded)
    );
    assert_eq!(
        captured(
            ConfigSourceKind::File,
            [(path("aa", "bb"), set(DocumentValue::Boolean(true)))],
            limits
        ),
        Err(ConfigError::CapacityExceeded)
    );
}

#[test]
fn value_total_counts_utf8_across_individually_valid_values() {
    let mut limits = ConfigLimits::W1;
    limits.max_decoded_value_bytes_total = 4;
    assert!(file("[a]\nx='é'\ny='é'\n", limits).is_ok());
    assert!(
        captured(
            ConfigSourceKind::Cli,
            [
                (path("a", "x"), set(DocumentValue::Text("é".to_owned()))),
                (path("a", "y"), set(DocumentValue::Text("é".to_owned()))),
            ],
            limits
        )
        .is_ok()
    );
    limits.max_decoded_value_bytes_total = 3;
    assert_eq!(
        file("[a]\nx='é'\ny='é'\n", limits),
        Err(ConfigError::CapacityExceeded)
    );
    assert_eq!(
        captured(
            ConfigSourceKind::Cli,
            [
                (path("a", "x"), set(DocumentValue::Text("é".to_owned()))),
                (path("a", "y"), set(DocumentValue::Text("é".to_owned()))),
            ],
            limits
        ),
        Err(ConfigError::CapacityExceeded)
    );
}

#[test]
fn scalar_widths_and_reset_have_exact_charges() {
    let mut limits = ConfigLimits::W1;
    limits.max_decoded_value_bytes_total = 9;
    limits.max_decoded_identifier_bytes_total = 7; // a1 + three a/key paths2 each
    let entries = || {
        [
            (path("a", "x"), set(DocumentValue::Boolean(false))),
            (path("a", "y"), set(DocumentValue::Integer(i64::MIN))),
            (path("a", "z"), LayerOperation::Reset),
        ]
    };
    assert!(captured(ConfigSourceKind::Environment, entries(), limits).is_ok());
    limits.max_decoded_value_bytes_total = 8;
    assert_eq!(
        captured(ConfigSourceKind::Environment, entries(), limits),
        Err(ConfigError::CapacityExceeded)
    );
    limits.max_decoded_identifier_bytes_total = ConfigLimits::W1.max_decoded_identifier_bytes_total;
    limits.max_decoded_value_bytes_total = 9;
    assert!(file("[a]\nx=false\ny=-9223372036854775808\n", limits).is_ok());
    limits.max_decoded_value_bytes_total = 8;
    assert_eq!(
        file("[a]\nx=false\ny=0\n", limits),
        Err(ConfigError::CapacityExceeded)
    );
}

#[test]
fn per_text_limit_measures_decoded_utf8() {
    let mut limits = ConfigLimits::W1;
    limits.max_text_bytes = 6;
    assert!(file("[a]\nx='ééé'\n", limits).is_ok());
    assert_eq!(
        file("[a]\nx='éééé'\n", limits),
        Err(ConfigError::CapacityExceeded)
    );
    assert_eq!(
        captured(
            ConfigSourceKind::Cli,
            [(path("a", "x"), set(DocumentValue::Text("éééé".to_owned())))],
            limits
        ),
        Err(ConfigError::CapacityExceeded)
    );
}

#[test]
fn shrinking_escapes_are_charged_after_counting_before_allocation() {
    let mut limits = ConfigLimits::W1;
    limits.max_text_bytes = 6;
    limits.max_decoded_value_bytes_total = 6;
    let parsed = file("[a]\nx=\"\\n\\n\\n\\n\\n\\n\"\n", limits).expect("decoded6 from raw12");
    let built = captured(
        ConfigSourceKind::File,
        [(path("a", "x"), set(DocumentValue::Text("\n".repeat(6))))],
        limits,
    )
    .expect("decoded capture");
    assert_eq!(parsed, built);
}

#[test]
fn global_list_limit_is_shared_across_lists() {
    let mut limits = ConfigLimits::W1;
    limits.max_list_items = 2;
    limits.max_total_list_items = 3;
    let entries = || {
        [
            (
                path("a", "x"),
                set(DocumentValue::StringList(vec![
                    String::new(),
                    String::new(),
                ])),
            ),
            (
                path("a", "y"),
                set(DocumentValue::StringList(vec![
                    String::new(),
                    String::new(),
                ])),
            ),
        ]
    };
    assert_eq!(
        file("[a]\nx=['','']\ny=['','']\n", limits),
        Err(ConfigError::CapacityExceeded)
    );
    assert_eq!(
        captured(ConfigSourceKind::Cli, entries(), limits),
        Err(ConfigError::CapacityExceeded)
    );
    limits.max_total_list_items = 4;
    assert!(file("[a]\nx=['','']\ny=['','']\n", limits).is_ok());
    assert!(captured(ConfigSourceKind::Cli, entries(), limits).is_ok());
}

#[test]
fn list_item_bytes_and_per_list_count_are_bounded() {
    let mut limits = ConfigLimits::W1;
    limits.max_list_items = 1;
    limits.max_text_bytes = 6;
    assert!(file("[a]\nx=['ééé']\n", limits).is_ok());
    assert_eq!(
        file("[a]\nx=['éééé']\n", limits),
        Err(ConfigError::CapacityExceeded)
    );
    assert_eq!(
        file("[a]\nx=['','']\n", limits),
        Err(ConfigError::CapacityExceeded)
    );
    assert_eq!(
        captured(
            ConfigSourceKind::Cli,
            [(
                path("a", "x"),
                set(DocumentValue::StringList(vec!["éééé".to_owned()]))
            )],
            limits
        ),
        Err(ConfigError::CapacityExceeded)
    );
    assert_eq!(
        captured(
            ConfigSourceKind::Cli,
            [(
                path("a", "x"),
                set(DocumentValue::StringList(vec![
                    String::new(),
                    String::new()
                ]))
            )],
            limits
        ),
        Err(ConfigError::CapacityExceeded)
    );
}

#[test]
fn captured_names_cannot_bypass_individual_identifier_ceiling() {
    let mut limits = ConfigLimits::W1;
    limits.max_identifier_bytes = 1;
    assert_eq!(
        captured(
            ConfigSourceKind::Cli,
            [(path("aa", "x"), LayerOperation::Reset)],
            limits
        ),
        Err(ConfigError::CapacityExceeded)
    );
    assert_eq!(
        file("[aa]\nx=0\n", limits),
        Err(ConfigError::CapacityExceeded)
    );
}

#[test]
fn duplicates_reject_without_a_document_or_overwrite() {
    assert_eq!(
        file("[a]\nx=1\nx=2\n", ConfigLimits::W1),
        Err(ConfigError::DuplicateKey)
    );
    assert_eq!(
        file("[a]\n[a]\n", ConfigLimits::W1),
        Err(ConfigError::DuplicateTable)
    );
    assert_eq!(
        file("profile='direct'\n", ConfigLimits::W1),
        Err(ConfigError::DuplicateKey)
    );
    assert_eq!(
        file("schema_version=2\n", ConfigLimits::W1),
        Err(ConfigError::DuplicateKey)
    );
    assert_eq!(
        captured(
            ConfigSourceKind::Cli,
            [
                (path("a", "x"), set(DocumentValue::Integer(1))),
                (path("a", "x"), set(DocumentValue::Integer(2))),
            ],
            ConfigLimits::W1
        ),
        Err(ConfigError::DuplicateKey)
    );
}

#[test]
fn header_semantics_stay_distinct_between_file_and_capture() {
    assert!(
        ConfigDocument::from_entries(
            1,
            None,
            source(ConfigSourceKind::File),
            [],
            ConfigLimits::W1
        )
        .is_ok()
    );
    assert_eq!(
        parse_document(
            b"schema_version=2\n",
            source(ConfigSourceKind::File),
            ConfigLimits::W1
        ),
        Err(ConfigError::ProfileNotAuthorized)
    );
    assert_eq!(
        ConfigDocument::from_entries(0, None, source(ConfigSourceKind::Cli), [], ConfigLimits::W1),
        Err(ConfigError::UnsupportedSchemaVersion)
    );
    assert_eq!(
        ConfigDocument::from_entries(
            1,
            None,
            source(ConfigSourceKind::Defaults),
            [],
            ConfigLimits::W1
        ),
        Err(ConfigError::SourceKindMismatch)
    );
    assert_eq!(
        ConfigDocument::from_entries(
            1,
            Some(ProfileId::new("direct").expect("profile")),
            source(ConfigSourceKind::Cli),
            [],
            ConfigLimits::W1
        ),
        Err(ConfigError::SourceKindMismatch)
    );
}

#[test]
fn all_new_zero_ceilings_reject_before_construction() {
    let mut variants = [ConfigLimits::W1; 3];
    variants[0].max_decoded_identifier_bytes_total = 0;
    variants[1].max_decoded_value_bytes_total = 0;
    variants[2].max_total_list_items = 0;
    for limits in variants {
        assert_eq!(file("[a]\nx=1\n", limits), Err(ConfigError::InvalidLimits));
        assert_eq!(
            captured(ConfigSourceKind::Cli, [], limits),
            Err(ConfigError::InvalidLimits)
        );
    }
}

#[test]
fn integer_extrema_and_maintained_decimal_grammar_are_preserved() {
    let parsed = file(
        "[a]\nx=-9223372036854775808\ny=9223372036854775807\nz=-1_000\n",
        ConfigLimits::W1,
    )
    .expect("maintained decimal integers");
    let built = captured(
        ConfigSourceKind::File,
        [
            (path("a", "x"), set(DocumentValue::Integer(i64::MIN))),
            (path("a", "y"), set(DocumentValue::Integer(i64::MAX))),
            (path("a", "z"), set(DocumentValue::Integer(-1_000))),
        ],
        ConfigLimits::W1,
    )
    .expect("captured extrema");
    assert_eq!(parsed, built);
    // Explicit syntax migration: the old handwritten parser accepted -_1.
    assert_eq!(
        file("[a]\nx=-_1\n", ConfigLimits::W1),
        Err(ConfigError::UnsupportedSyntax)
    );
    assert_eq!(
        file("[a]\nx=9223372036854775808\n", ConfigLimits::W1),
        Err(ConfigError::ValueOutOfBounds)
    );
    assert_eq!(
        file("[a]\nx=-9223372036854775809\n", ConfigLimits::W1),
        Err(ConfigError::ValueOutOfBounds)
    );
    assert_eq!(
        file("[a]\nx=1__0\n", ConfigLimits::W1),
        Err(ConfigError::UnsupportedSyntax)
    );
}

#[test]
fn capacity_errors_contain_no_value_or_source_text() {
    let mut limits = ConfigLimits::W1;
    limits.max_decoded_value_bytes_total = 1;
    let error = file("[a]\nx='private-marker'\n", limits).expect_err("capacity");
    assert_eq!(error.to_string(), "configuration capacity exceeded");
    assert!(!format!("{error:?}").contains("private-marker"));
}
