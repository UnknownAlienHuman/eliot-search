//! Document Debug disclosure fixtures. AUTHORED_NOT_RUN; integrated #238/#215 qualification.

use search_config::{
    ConfigDocument, ConfigKeyName, ConfigKeyPath, ConfigLayers, ConfigLimits, ConfigSectionName,
    ConfigSource, ConfigSourceKind, ConfigSourceRef, DocumentValue, LayerOperation, parse_document,
};
use search_contracts::{Blake3Digest32, ProfileId};

const PROFILE_MARKER: &str = "private_profile_é_marker";
const VALUE_MARKER: &str = "private-value-marker";

fn source(kind: ConfigSourceKind) -> ConfigSource {
    ConfigSource {
        kind,
        source_ref: ConfigSourceRef::new("private-source-marker", 64).expect("source"),
        source_digest: Blake3Digest32::from_bytes([1; 32]),
    }
}

fn path(key: &str) -> ConfigKeyPath {
    ConfigKeyPath::new(
        ConfigSectionName::new("private_section_marker", 128).expect("section"),
        ConfigKeyName::new(key, 128).expect("key"),
    )
}

fn captured(kind: ConfigSourceKind) -> ConfigDocument {
    ConfigDocument::from_entries(
        7,
        (kind == ConfigSourceKind::File).then(|| ProfileId::new(PROFILE_MARKER).expect("profile")),
        source(kind),
        [(
            path("private_key_marker"),
            LayerOperation::Set(DocumentValue::Text(VALUE_MARKER.to_owned())),
        )],
        ConfigLimits::W1,
    )
    .expect("captured")
}

fn debug_forms(value: &impl std::fmt::Debug) -> [String; 2] {
    [format!("{value:?}"), format!("{value:#?}")]
}

#[test]
fn captured_and_parsed_profile_text_is_absent_in_both_debug_forms() {
    let bytes = format!("schema_version=7\nprofile='{PROFILE_MARKER}'\n");
    let parsed = parse_document(
        bytes.as_bytes(),
        source(ConfigSourceKind::File),
        ConfigLimits::W1,
    )
    .expect("parsed profile is bounded but not yet authorized");
    for document in [captured(ConfigSourceKind::File), parsed] {
        assert_eq!(
            document.requested_profile().expect("profile").as_str(),
            PROFILE_MARKER
        );
        for rendered in debug_forms(&document) {
            assert!(!rendered.contains(PROFILE_MARKER));
            assert!(rendered.contains("requested_profile_present: true"));
            let byte_count = rendered
                .split_once("requested_profile_bytes: Some(")
                .expect("profile byte metadata")
                .1
                .split_once(')')
                .expect("profile metadata close")
                .0
                .trim()
                .trim_end_matches(',')
                .parse::<usize>()
                .expect("profile byte count");
            assert_eq!(byte_count, PROFILE_MARKER.len());
        }
    }
}

#[test]
fn document_debug_omits_payload_and_preserves_typed_data() {
    let document = captured(ConfigSourceKind::File);
    assert_eq!(document, document.clone());
    assert_eq!(document.schema_version(), 7);
    assert_eq!(document.source(), &source(ConfigSourceKind::File));
    assert_eq!(document.len(), 1);
    assert!(!document.is_empty());
    assert_eq!(
        document
            .declared_sections()
            .next()
            .expect("section")
            .as_str(),
        "private_section_marker"
    );
    let (key, operation) = document.entries().next().expect("entry");
    assert_eq!(key, &path("private_key_marker"));
    assert_eq!(
        operation,
        &LayerOperation::Set(DocumentValue::Text(VALUE_MARKER.to_owned()))
    );
    for rendered in debug_forms(&document) {
        for marker in [
            PROFILE_MARKER,
            VALUE_MARKER,
            "private-source-marker",
            "private_section_marker",
            "private_key_marker",
            "source_digest",
            "source_ref",
        ] {
            assert!(!rendered.contains(marker));
        }
        assert!(rendered.contains("schema_version: 7"));
        assert!(rendered.contains("source_kind: File"));
        assert!(rendered.contains("entry_count: 1"));
        assert!(rendered.contains("declared_section_count: 1"));
    }
}

#[test]
fn containing_layers_hide_captured_documents_and_keep_selected_profile() {
    let layers = ConfigLayers {
        defaults: source(ConfigSourceKind::Defaults),
        requested_profile: ProfileId::new("direct").expect("trusted selected profile"),
        file: Some(captured(ConfigSourceKind::File)),
        environment: Some(captured(ConfigSourceKind::Environment)),
        cli: Some(captured(ConfigSourceKind::Cli)),
    };
    for rendered in debug_forms(&layers) {
        for marker in [
            PROFILE_MARKER,
            VALUE_MARKER,
            "private-source-marker",
            "private_section_marker",
            "private_key_marker",
        ] {
            assert!(!rendered.contains(marker));
        }
        assert!(rendered.contains("direct"));
        for kind in ["File", "Environment", "Cli"] {
            assert!(rendered.contains(&format!("source_kind: {kind}")));
        }
    }
}

#[test]
fn empty_captured_documents_keep_none_profile_and_zero_counts() {
    for kind in [
        ConfigSourceKind::File,
        ConfigSourceKind::Environment,
        ConfigSourceKind::Cli,
    ] {
        let document = ConfigDocument::from_entries(1, None, source(kind), [], ConfigLimits::W1)
            .expect("empty captured document");
        assert!(document.requested_profile().is_none());
        assert!(document.is_empty());
        assert_eq!(document.entries().len(), 0);
        assert_eq!(document.declared_sections().len(), 0);
        for rendered in debug_forms(&document) {
            assert!(rendered.contains("requested_profile_present: false"));
            assert!(rendered.contains("requested_profile_bytes: None"));
            assert!(rendered.contains("entry_count: 0"));
            assert!(rendered.contains("declared_section_count: 0"));
        }
    }
}

#[test]
fn fixed_debug_context_depends_on_counts_not_payload_size() {
    let document = |entries: Vec<(ConfigKeyPath, LayerOperation)>| {
        ConfigDocument::from_entries(
            1,
            None,
            source(ConfigSourceKind::File),
            entries,
            ConfigLimits::W1,
        )
        .expect("bounded captured document")
    };
    let short = document(vec![(
        path("k0"),
        LayerOperation::Set(DocumentValue::Text("x".to_owned())),
    )]);
    let long = document(vec![(
        path("k0"),
        LayerOperation::Set(DocumentValue::Text("x".repeat(65_536))),
    )]);
    let many = document(
        (0..256)
            .map(|index| {
                (
                    path(&format!("k{index}")),
                    LayerOperation::Set(DocumentValue::Text(VALUE_MARKER.to_owned())),
                )
            })
            .collect(),
    );
    assert_eq!(debug_forms(&short), debug_forms(&long));
    for (small, large) in debug_forms(&short).into_iter().zip(debug_forms(&many)) {
        // The only representation delta is the entry count's decimal width: 1 -> 256.
        assert_eq!(large.len(), small.len() + 2);
        assert!(!large.contains(VALUE_MARKER));
    }
}
