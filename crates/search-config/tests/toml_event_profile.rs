//! #400 causal FILE-profile fixtures. AUTHORED_NOT_RUN under #352/#215.

use std::num::NonZeroU64;

use search_config::{
    CONFIG_TOML_PROFILE, CONFIG_TOML_SCHEMA_VERSION, ConfigDocument, ConfigError,
    ConfigFieldDescriptor, ConfigKeyName, ConfigKeyPath, ConfigLayers, ConfigLimits, ConfigOwner,
    ConfigRegistry, ConfigSectionDescriptor, ConfigSectionName, ConfigSource, ConfigSourceKind,
    ConfigSourceRef, ConfigValue, ConfigValueKind, DocumentValue, EffectiveConfigSnapshot,
    LayerOperation, ReconfigurationAction, RedactionPolicy, ReloadClass, SecretPolicy,
    SecurityFloor, ValidatedSection, ValueBounds, assemble_effective, diff, merge_layers,
    parse_document, project_section, register_sections,
};
use search_contracts::{Blake3Digest32, ProfileId};

fn source(kind: ConfigSourceKind) -> ConfigSource {
    ConfigSource {
        kind,
        source_ref: ConfigSourceRef::new("synthetic-source-private", 64).expect("ref"),
        source_digest: Blake3Digest32::from_bytes([7; 32]),
    }
}

fn parse(bytes: &[u8], limits: ConfigLimits) -> Result<ConfigDocument, ConfigError> {
    parse_document(bytes, source(ConfigSourceKind::File), limits)
}

fn file(tail: &str, limits: ConfigLimits) -> Result<ConfigDocument, ConfigError> {
    parse(
        format!("schema_version=2\nprofile='direct'\n{tail}").as_bytes(),
        limits,
    )
}

fn path(key: &str) -> ConfigKeyPath {
    ConfigKeyPath::new(
        ConfigSectionName::new("cfg", 128).expect("section"),
        ConfigKeyName::new(key, 128).expect("key"),
    )
}

#[test]
fn maintained_plus_unicode_and_multiline_array_decode_to_typed_values() {
    let parsed = file(
        r#"[cfg]
number=+1_000
text="\u00e9"
list=[
  'a', # donor whitespace/comment grammar
  "b\u00e9",
]
"#,
        ConfigLimits::W1,
    )
    .expect("new maintained spellings");
    let captured = ConfigDocument::from_entries(
        2,
        Some(ProfileId::new("direct").expect("profile")),
        source(ConfigSourceKind::File),
        [
            (
                path("number"),
                LayerOperation::Set(DocumentValue::Integer(1_000)),
            ),
            (
                path("text"),
                LayerOperation::Set(DocumentValue::Text("é".into())),
            ),
            (
                path("list"),
                LayerOperation::Set(DocumentValue::StringList(vec!["a".into(), "bé".into()])),
            ),
        ],
        ConfigLimits::W1,
    )
    .expect("same typed values and supplied provenance");
    assert_eq!(parsed, captured);
}

#[test]
fn schema1_requires_migration_but_typed_constructor_makes_no_syntax_claim() {
    assert_eq!(CONFIG_TOML_SCHEMA_VERSION, 2);
    assert_eq!(CONFIG_TOML_PROFILE, "eliot-config-toml@2");
    assert_eq!(
        parse(b"schema_version=1\nprofile='direct'\n", ConfigLimits::W1),
        Err(ConfigError::UnsupportedSchemaVersion)
    );
    assert_eq!(
        file("", ConfigLimits::W1)
            .expect("schema2")
            .schema_version(),
        2
    );
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
        file("[cfg]\nx=-_1\n", ConfigLimits::W1),
        Err(ConfigError::UnsupportedSyntax)
    );
}

#[test]
fn trailing_comma_is_retained_and_multiple_flat_tables_do_not_consume_value_depth() {
    let plain = file("[cfg]\nx=['a','b']\n", ConfigLimits::W1).expect("plain");
    let comma = file("[cfg]\nx=['a','b',]\n", ConfigLimits::W1).expect("retained trailing comma");
    assert_eq!(plain, comma);
    assert_eq!(
        file("[cfg]\nx=[]\n[other]\ny=['']\n", ConfigLimits::W1)
            .expect("two flat tables")
            .len(),
        2
    );
}

#[test]
fn root_duplicates_reserved_tables_and_scoped_fields_are_distinct() {
    for tail in ["schema_version=2\n", "profile='direct'\n"] {
        assert_eq!(file(tail, ConfigLimits::W1), Err(ConfigError::DuplicateKey));
    }
    for tail in ["[profile]\n", "[schema_version]\n", "[cfg]\n[cfg]\n"] {
        assert_eq!(
            file(tail, ConfigLimits::W1),
            Err(ConfigError::DuplicateTable)
        );
    }
    assert_eq!(
        file("[cfg]\nx=1\nx=2\n", ConfigLimits::W1),
        Err(ConfigError::DuplicateKey)
    );
    assert_eq!(
        file("unknown=1\n", ConfigLimits::W1),
        Err(ConfigError::UnknownKey)
    );
    assert_eq!(
        parse(
            b"schema_version=2\n[cfg]\nprofile='direct'\n",
            ConfigLimits::W1
        ),
        Err(ConfigError::ProfileNotAuthorized)
    );
    assert_eq!(
        parse(b"profile='direct'\n", ConfigLimits::W1),
        Err(ConfigError::UnsupportedSchemaVersion)
    );
    assert_eq!(
        file(
            "[cfg]\nprofile='direct'\nschema_version=2\n",
            ConfigLimits::W1
        )
        .expect("scoped fields")
        .len(),
        2
    );
}

#[test]
fn unsupported_key_table_and_value_shapes_fail_closed() {
    for tail in [
        "[cfg]\n'x'=1\n",
        "['cfg']\nx=1\n",
        "[cfg.sub]\nx=1\n",
        "[cfg]\nx.y=1\n",
        "[[cfg]]\nx=1\n",
        "[cfg]\nx={a='b'}\n",
        "[cfg]\nx=[['a']]\n",
        "[cfg]\nx=['a',true]\n",
        "[cfg]\nx=0x10\n",
        "[cfg]\nx=0o10\n",
        "[cfg]\nx=0b10\n",
        "[cfg]\nx=1.5\n",
        "[cfg]\nx=2026-10-10\n",
        "[cfg]\nx='''multi'''\n",
    ] {
        assert!(
            file(tail, ConfigLimits::W1).is_err(),
            "unsupported fixture shape"
        );
    }
    let deep = format!("[cfg]\nx={}{}\n", "[".repeat(64), "]".repeat(64));
    assert!(file(&deep, ConfigLimits::W1).is_err());
}

#[test]
fn malformed_donor_input_cannot_recover_into_a_partial_document() {
    for tail in [
        "[cfg]\nx=['private-marker',1]\n[other]\ny='ok'\n",
        "[cfg]\nx=['private-marker'\n[other]\ny='ok'\n",
        "[cfg]\nx=\"private-\\q-marker\"\n[other]\ny='ok'\n",
        "[cfg]\nx=\"\\uD800\"\n[other]\ny='ok'\n",
        "[cfg]\nx 1\n[other]\ny='ok'\n",
    ] {
        let error = file(tail, ConfigLimits::W1).expect_err("no recovery success");
        assert!(!error.to_string().contains("private-marker"));
        assert!(!format!("{error:?}").contains("synthetic-source-private"));
    }
}

#[test]
fn framing_has_exact_lf_crlf_final_segment_and_lone_cr_rules() {
    let limits = ConfigLimits {
        max_lines: 2,
        ..ConfigLimits::W1
    };
    for text in [
        "schema_version=2\nprofile='direct'",
        "schema_version=2\nprofile='direct'\n",
        "schema_version=2\r\nprofile='direct'",
        "schema_version=2\r\nprofile='direct'\r\n",
    ] {
        assert!(parse(text.as_bytes(), limits).is_ok());
    }
    assert_eq!(
        parse(b"schema_version=2\nprofile='direct'\n\n", limits),
        Err(ConfigError::CapacityExceeded)
    );
    for text in [
        "schema_version=2\rprofile='direct'",
        "schema_version=2\nprofile='direct'\r",
    ] {
        assert_eq!(
            parse(text.as_bytes(), ConfigLimits::W1),
            Err(ConfigError::UnsupportedSyntax)
        );
    }
    assert_eq!(parse(&[], ConfigLimits::W1), Err(ConfigError::EmptyInput));
    assert_eq!(
        parse(&[0xff], ConfigLimits::W1),
        Err(ConfigError::InvalidUtf8)
    );
    assert_eq!(
        parse(&[0xef, 0xbb, 0xbf, b'x'], ConfigLimits::W1),
        Err(ConfigError::BomForbidden)
    );
    let bytes = b"schema_version=2\nprofile='direct'\n";
    for kind in [
        ConfigSourceKind::Defaults,
        ConfigSourceKind::Environment,
        ConfigSourceKind::Cli,
    ] {
        assert_eq!(
            parse_document(bytes, source(kind), ConfigLimits::W1),
            Err(ConfigError::SourceKindMismatch)
        );
    }
    assert!(
        parse(
            bytes,
            ConfigLimits {
                max_document_bytes: bytes.len(),
                ..ConfigLimits::W1
            }
        )
        .is_ok()
    );
    assert_eq!(
        parse(
            bytes,
            ConfigLimits {
                max_document_bytes: bytes.len() - 1,
                ..ConfigLimits::W1
            }
        ),
        Err(ConfigError::CapacityExceeded)
    );
}

#[test]
fn exact_token_count_and_storage_ceilings_are_independent() {
    let text = "schema_version=2\nprofile='direct'\n";
    let count = toml_parser::Source::new(text).lex().count();
    let storage = count * size_of::<toml_parser::lexer::Token>();
    let exact = ConfigLimits {
        max_lex_tokens: count,
        max_lex_token_storage_bytes: storage,
        ..ConfigLimits::W1
    };
    assert!(parse(text.as_bytes(), exact).is_ok());
    assert_eq!(
        parse(
            text.as_bytes(),
            ConfigLimits {
                max_lex_tokens: count - 1,
                ..exact
            }
        ),
        Err(ConfigError::CapacityExceeded)
    );
    assert_eq!(
        parse(
            text.as_bytes(),
            ConfigLimits {
                max_lex_token_storage_bytes: storage - 1,
                ..exact
            }
        ),
        Err(ConfigError::CapacityExceeded)
    );
    for limits in [
        ConfigLimits {
            max_lex_tokens: 0,
            ..exact
        },
        ConfigLimits {
            max_lex_token_storage_bytes: 0,
            ..exact
        },
    ] {
        assert_eq!(
            parse(text.as_bytes(), limits),
            Err(ConfigError::InvalidLimits)
        );
        assert_eq!(
            ConfigDocument::from_entries(2, None, source(ConfigSourceKind::Cli), [], limits),
            Err(ConfigError::InvalidLimits)
        );
    }
}

#[test]
fn decoded_utf8_and_identifier_extents_are_charged_before_owned_growth() {
    let exact = ConfigLimits {
        max_text_bytes: 6,
        max_decoded_value_bytes_total: 6,
        max_decoded_identifier_bytes_total: 13, // direct6 + table cfg3 + entry cfg3/x1
        ..ConfigLimits::W1
    };
    assert!(file("[cfg]\nx=\"\\u00e9\\u00e9\\u00e9\"\n", exact).is_ok());
    assert_eq!(
        file(
            "[cfg]\nx='ééé'\n",
            ConfigLimits {
                max_decoded_identifier_bytes_total: 12,
                ..exact
            }
        ),
        Err(ConfigError::CapacityExceeded)
    );
    assert_eq!(
        file(
            "[cfg]\nx='ééé'\n",
            ConfigLimits {
                max_decoded_value_bytes_total: 5,
                ..exact
            }
        ),
        Err(ConfigError::CapacityExceeded)
    );
    assert_eq!(
        file(
            "[cfg]\nx='éééé'\n",
            ConfigLimits {
                max_text_bytes: 6,
                ..ConfigLimits::W1
            }
        ),
        Err(ConfigError::CapacityExceeded)
    );
}

#[test]
fn list_and_collection_limits_reject_without_a_partial_result_or_disclosure() {
    let limits = ConfigLimits {
        max_list_items: 2,
        max_total_list_items: 3,
        ..ConfigLimits::W1
    };
    let text = "[cfg]\nx=['','']\ny=['']\n";
    assert!(file(text, limits).is_ok());
    assert_eq!(
        file(
            text,
            ConfigLimits {
                max_total_list_items: 2,
                ..limits
            }
        ),
        Err(ConfigError::CapacityExceeded)
    );
    assert_eq!(
        file(
            text,
            ConfigLimits {
                max_list_items: 1,
                ..limits
            }
        ),
        Err(ConfigError::CapacityExceeded)
    );
    assert_eq!(
        file(
            "[cfg]\n[other]\n",
            ConfigLimits {
                max_sections: 1,
                ..ConfigLimits::W1
            }
        ),
        Err(ConfigError::CapacityExceeded)
    );
    for limits in [
        ConfigLimits {
            max_entries_per_layer: 1,
            ..ConfigLimits::W1
        },
        ConfigLimits {
            max_fields_per_section: 1,
            ..ConfigLimits::W1
        },
    ] {
        assert_eq!(
            file("[cfg]\nx=true\ny=false\n", limits),
            Err(ConfigError::CapacityExceeded)
        );
    }
    let error = file(
        "[cfg]\nx='private-marker'\ny='ok'\n",
        ConfigLimits {
            max_decoded_value_bytes_total: 1,
            ..ConfigLimits::W1
        },
    )
    .expect_err("no partial document");
    assert_eq!(error, ConfigError::CapacityExceeded);
    assert!(!error.to_string().contains("private-marker"));
    let document = file("[cfg]\nx='private-marker'\n", ConfigLimits::W1).expect("captured");
    let debug = format!("{document:?}");
    assert!(!debug.contains("private-marker"));
    assert!(!debug.contains("synthetic-source-private"));
}

fn snapshot(schema: u32) -> (ConfigRegistry, EffectiveConfigSnapshot) {
    let field = ConfigFieldDescriptor::new(
        ConfigKeyName::new("enabled", 128).expect("key"),
        ConfigValueKind::Boolean,
        ConfigValue::Boolean(true),
        ValueBounds {
            max_text_bytes: 64,
            max_list_items: 4,
            max_list_item_bytes: 64,
            integer_min: i64::MIN,
            integer_max: i64::MAX,
        },
        [ConfigSourceKind::File],
        false,
        SecurityFloor::None,
        RedactionPolicy::Public,
        [ReconfigurationAction::ApplyLive],
    )
    .expect("descriptor field");
    let descriptor = ConfigSectionDescriptor::new(
        ConfigSectionName::new("cfg", 128).expect("section"),
        ConfigOwner::new("fixture", 128).expect("owner"),
        NonZeroU64::new(1).expect("revision"),
        ReloadClass::ApplyLive,
        Blake3Digest32::from_bytes([1; 32]),
        SecretPolicy::ForbidPlaintext,
        [field],
        ConfigLimits::W1,
    )
    .expect("section descriptor");
    let registry = register_sections(schema, [descriptor], ConfigLimits::W1).expect("registry");
    let profile = ProfileId::new("direct").expect("profile");
    let merged = merge_layers(
        ConfigLayers {
            defaults: source(ConfigSourceKind::Defaults),
            requested_profile: profile.clone(),
            file: None,
            environment: None,
            cli: None,
        },
        &registry,
        ConfigLimits::W1,
    )
    .expect("defaults");
    let name = ConfigSectionName::new("cfg", 128).expect("section");
    let input =
        project_section(&merged, registry.section(&name).expect("descriptor")).expect("projected");
    let validated = ValidatedSection::new(input, profile.clone(), ConfigLimits::W1)
        .expect("equality identity only");
    let snapshot =
        assemble_effective(&registry, [validated], profile, ConfigLimits::W1).expect("snapshot");
    (registry, snapshot)
}

#[test]
fn schema2_binds_snapshot_header_while_canonical_domain_and_tag_stay_fixed() {
    let (_, old) = snapshot(1);
    let (registry, current) = snapshot(2);
    let name = ConfigSectionName::new("cfg", 128).expect("section");
    assert_eq!(
        old.section(&name)
            .expect("old section")
            .validated()
            .validated_section_digest(),
        current
            .section(&name)
            .expect("current section")
            .validated()
            .validated_section_digest(),
    );
    assert_ne!(old.fingerprint(), current.fingerprint());
    for fingerprint in [old.fingerprint(), current.fingerprint()] {
        assert_eq!(fingerprint.algorithm(), "blake3");
        assert_eq!(
            fingerprint.canonical_profile(),
            "eliot/cbor/config-snapshot/v2"
        );
        assert_eq!(fingerprint.revision(), 2);
    }
    assert_eq!(
        diff(&old, &current, &registry),
        Err(ConfigError::UnsupportedSchemaVersion)
    );
}

#[test]
fn section_fields_named_like_headers_reach_registry_unknown_key_validation() {
    let (registry, _) = snapshot(2);
    let document = file(
        "[cfg]\nprofile='direct'\nschema_version=2\n",
        ConfigLimits::W1,
    )
    .expect("scoped fields");
    let merged = merge_layers(
        ConfigLayers {
            defaults: source(ConfigSourceKind::Defaults),
            requested_profile: ProfileId::new("direct").expect("profile"),
            file: Some(document),
            environment: None,
            cli: None,
        },
        &registry,
        ConfigLimits::W1,
    );
    assert_eq!(merged, Err(ConfigError::UnknownKey));
}
