use std::collections::BTreeSet;
use std::num::NonZeroU64;

use search_config::{
    ConfigDocument, ConfigError, ConfigFieldDescriptor, ConfigKeyName, ConfigKeyPath, ConfigLayers,
    ConfigLimits, ConfigOwner, ConfigRegistry, ConfigSectionDescriptor, ConfigSectionName,
    ConfigSource, ConfigSourceKind, ConfigSourceRef, ConfigValue, ConfigValueKind, DisclosureLevel,
    DocumentValue, EffectiveConfigSnapshot, LayerOperation, PathLocationClass,
    ReconfigurationAction, RedactedValue, ReloadClass, SecretPolicy, SecurityFloor,
    ValidatedSection, ValueBounds, assemble_effective, diff, merge_layers, parse_document,
    plan_reconfiguration, project_section, redacted_view, register_sections,
    validate_environment_key,
};
use search_contracts::{Blake3Digest32, ProfileId};

const fn limits() -> ConfigLimits {
    ConfigLimits {
        max_document_bytes: 64 * 1024,
        max_lines: 1_024,
        max_sections: 32,
        max_fields_per_section: 64,
        max_entries_per_layer: 256,
        max_identifier_bytes: 128,
        max_text_bytes: 4_096,
        max_list_items: 64,
        max_canonical_bytes: 64 * 1024,
        max_diagnostic_entries: 256,
    }
}

fn source(kind: ConfigSourceKind, marker: u8) -> ConfigSource {
    ConfigSource {
        kind,
        source_ref: ConfigSourceRef::new(format!("captured-{marker}"), 128).expect("source"),
        source_digest: Blake3Digest32::from_bytes([marker; 32]),
    }
}

fn name(value: &str) -> ConfigSectionName {
    ConfigSectionName::new(value, 128).expect("section")
}

fn key(value: &str) -> ConfigKeyName {
    ConfigKeyName::new(value, 128).expect("key")
}

fn path(section: &str, field: &str) -> ConfigKeyPath {
    ConfigKeyPath::new(name(section), key(field))
}

const fn bounds() -> ValueBounds {
    ValueBounds {
        max_text_bytes: 4_096,
        max_list_items: 64,
        max_list_item_bytes: 512,
        integer_min: 0,
        integer_max: 16_777_216,
    }
}

#[allow(clippy::too_many_arguments)]
fn field(
    field_name: &str,
    kind: ConfigValueKind,
    default: ConfigValue,
    allowed_sources: &[ConfigSourceKind],
    reset_allowed: bool,
    security_floor: SecurityFloor,
    redaction: search_config::RedactionPolicy,
    actions: &[ReconfigurationAction],
) -> ConfigFieldDescriptor {
    ConfigFieldDescriptor::new(
        key(field_name),
        kind,
        default,
        bounds(),
        allowed_sources.iter().copied(),
        reset_allowed,
        security_floor,
        redaction,
        actions.iter().copied(),
    )
    .expect("field descriptor")
}

fn section(
    section_name: &str,
    owner: &str,
    revision: u64,
    minimum_reload: ReloadClass,
    secret_policy: SecretPolicy,
    digest_marker: u8,
    fields: Vec<ConfigFieldDescriptor>,
) -> ConfigSectionDescriptor {
    ConfigSectionDescriptor::new(
        name(section_name),
        ConfigOwner::new(owner, 128).expect("owner"),
        NonZeroU64::new(revision).expect("revision"),
        minimum_reload,
        Blake3Digest32::from_bytes([digest_marker; 32]),
        secret_policy,
        fields,
        limits(),
    )
    .expect("section descriptor")
}

#[allow(clippy::too_many_lines)]
fn fixture_registry(revision: u64) -> ConfigRegistry {
    register_sections(
        1,
        [
            section(
                "instance",
                "search-runtime-owner",
                revision,
                ReloadClass::DrainAndRestart,
                SecretPolicy::ForbidPlaintext,
                1,
                vec![
                    field(
                        "mode",
                        ConfigValueKind::Text,
                        ConfigValue::Text("standalone".into()),
                        &[ConfigSourceKind::File, ConfigSourceKind::Cli],
                        true,
                        SecurityFloor::None,
                        search_config::RedactionPolicy::Public,
                        &[ReconfigurationAction::DrainAndRestart],
                    ),
                    field(
                        "data_root",
                        ConfigValueKind::Text,
                        ConfigValue::Absent,
                        &[
                            ConfigSourceKind::File,
                            ConfigSourceKind::Environment,
                            ConfigSourceKind::Cli,
                        ],
                        true,
                        SecurityFloor::None,
                        search_config::RedactionPolicy::PathDigest,
                        &[ReconfigurationAction::DrainAndRestart],
                    ),
                ],
            ),
            section(
                "secrets",
                "search-os-secrets",
                revision,
                ReloadClass::RestartDependency,
                SecretPolicy::OpaqueReferencesOnly,
                2,
                vec![field(
                    "qdrant_api_secret_ref",
                    ConfigValueKind::SecretReference,
                    ConfigValue::Absent,
                    &[ConfigSourceKind::File, ConfigSourceKind::Environment],
                    true,
                    SecurityFloor::None,
                    search_config::RedactionPolicy::Secret,
                    &[ReconfigurationAction::RestartDependency],
                )],
            ),
            section(
                "control",
                "search-control-redb",
                revision,
                ReloadClass::RestartDependency,
                SecretPolicy::ForbidPlaintext,
                3,
                vec![field(
                    "durability",
                    ConfigValueKind::Text,
                    ConfigValue::Text("fsync_atomic".into()),
                    &[ConfigSourceKind::File],
                    false,
                    SecurityFloor::Fixed,
                    search_config::RedactionPolicy::Public,
                    &[ReconfigurationAction::Reject],
                )],
            ),
            section(
                "source_admission",
                "search-source-admission",
                revision,
                ReloadClass::SecurityBarrier,
                SecretPolicy::ForbidPlaintext,
                4,
                vec![field(
                    "allow_generated",
                    ConfigValueKind::Boolean,
                    ConfigValue::Boolean(true),
                    &[ConfigSourceKind::File],
                    true,
                    SecurityFloor::BooleanMayOnlyRestrict,
                    search_config::RedactionPolicy::Public,
                    &[ReconfigurationAction::SecurityBarrier],
                )],
            ),
            section(
                "lexical",
                "search-lexical",
                revision,
                ReloadClass::NewCollectionGeneration,
                SecretPolicy::ForbidPlaintext,
                5,
                vec![field(
                    "profile_id",
                    ConfigValueKind::Text,
                    ConfigValue::Text("baseline-v1".into()),
                    &[ConfigSourceKind::File],
                    true,
                    SecurityFloor::None,
                    search_config::RedactionPolicy::Public,
                    &[
                        ReconfigurationAction::NewCollectionGeneration,
                        ReconfigurationAction::RebuildProjection,
                    ],
                )],
            ),
            section(
                "optional_profiles",
                "eliot-searchd",
                revision,
                ReloadClass::GateRequired,
                SecretPolicy::ForbidPlaintext,
                6,
                vec![field(
                    "semantic",
                    ConfigValueKind::Boolean,
                    ConfigValue::Boolean(false),
                    &[ConfigSourceKind::File],
                    true,
                    SecurityFloor::None,
                    search_config::RedactionPolicy::Public,
                    &[ReconfigurationAction::GateRequired],
                )],
            ),
        ],
        limits(),
    )
    .expect("registry")
}

fn document(
    kind: ConfigSourceKind,
    marker: u8,
    entries: impl IntoIterator<Item = (ConfigKeyPath, LayerOperation)>,
) -> ConfigDocument {
    ConfigDocument::from_entries(
        1,
        (kind == ConfigSourceKind::File).then(|| ProfileId::new("direct").expect("profile")),
        source(kind, marker),
        entries,
        limits(),
    )
    .expect("document")
}

fn snapshot(
    registry: &ConfigRegistry,
    file: Option<ConfigDocument>,
    environment: Option<ConfigDocument>,
    cli: Option<ConfigDocument>,
    requested_profile: &str,
    selected_profile: &str,
    digest_seed: u8,
) -> EffectiveConfigSnapshot {
    let merged = merge_layers(
        ConfigLayers {
            defaults: source(ConfigSourceKind::Defaults, 0),
            requested_profile: ProfileId::new(requested_profile).expect("requested profile"),
            file,
            environment,
            cli,
        },
        registry,
        limits(),
    )
    .expect("merged config");
    let selected_profile = ProfileId::new(selected_profile).expect("selected profile");
    let validated = registry
        .sections()
        .enumerate()
        .map(|(index, (_, descriptor))| {
            let input = project_section(&merged, descriptor).expect("section projection");
            let marker = digest_seed.wrapping_add(u8::try_from(index).expect("small fixture"));
            ValidatedSection::new(
                input,
                selected_profile.clone(),
                Blake3Digest32::from_bytes([marker; 32]),
            )
        })
        .collect::<Vec<_>>();
    assemble_effective(registry, validated, selected_profile, limits()).expect("effective snapshot")
}

#[test]
fn same_layers_same_fingerprint() {
    let registry = fixture_registry(1);
    let first = document(
        ConfigSourceKind::File,
        1,
        [
            (
                path("instance", "mode"),
                LayerOperation::Set(DocumentValue::Text("managed_client".into())),
            ),
            (
                path("source_admission", "allow_generated"),
                LayerOperation::Set(DocumentValue::Boolean(false)),
            ),
        ],
    );
    let second = document(
        ConfigSourceKind::File,
        1,
        [
            (
                path("source_admission", "allow_generated"),
                LayerOperation::Set(DocumentValue::Boolean(false)),
            ),
            (
                path("instance", "mode"),
                LayerOperation::Set(DocumentValue::Text("managed_client".into())),
            ),
        ],
    );
    let left = snapshot(&registry, Some(first), None, None, "direct", "direct", 20);
    let right = snapshot(&registry, Some(second), None, None, "direct", "direct", 20);
    assert_eq!(left, right);
    assert_eq!(left.fingerprint(), right.fingerprint());
}

#[test]
fn duplicate_unknown_and_wrong_type_fail_closed() {
    let registry = fixture_registry(1);
    assert_eq!(
        parse_document(
            b"schema_version=1\nprofile='direct'\n[instance]\nmode='a'\nmode='b'\n",
            source(ConfigSourceKind::File, 1),
            limits(),
        ),
        Err(ConfigError::DuplicateKey)
    );

    let unknown = document(
        ConfigSourceKind::File,
        1,
        [(
            path("instance", "typo"),
            LayerOperation::Set(DocumentValue::Text("value".into())),
        )],
    );
    assert_eq!(
        merge_layers(
            ConfigLayers {
                defaults: source(ConfigSourceKind::Defaults, 0),
                requested_profile: ProfileId::new("direct").expect("profile"),
                file: Some(unknown),
                environment: None,
                cli: None,
            },
            &registry,
            limits(),
        ),
        Err(ConfigError::UnknownKey)
    );

    let wrong_type = document(
        ConfigSourceKind::File,
        2,
        [(
            path("source_admission", "allow_generated"),
            LayerOperation::Set(DocumentValue::Integer(1)),
        )],
    );
    assert_eq!(
        merge_layers(
            ConfigLayers {
                defaults: source(ConfigSourceKind::Defaults, 0),
                requested_profile: ProfileId::new("direct").expect("profile"),
                file: Some(wrong_type),
                environment: None,
                cli: None,
            },
            &registry,
            limits(),
        ),
        Err(ConfigError::TypeMismatch)
    );
}

#[test]
fn override_allowlist_enforced() {
    let registry = fixture_registry(1);
    let environment = document(
        ConfigSourceKind::Environment,
        2,
        [(
            path("instance", "mode"),
            LayerOperation::Set(DocumentValue::Text("managed_client".into())),
        )],
    );
    assert_eq!(
        merge_layers(
            ConfigLayers {
                defaults: source(ConfigSourceKind::Defaults, 0),
                requested_profile: ProfileId::new("direct").expect("profile"),
                file: None,
                environment: Some(environment),
                cli: None,
            },
            &registry,
            limits(),
        ),
        Err(ConfigError::OverrideNotAllowed)
    );
}

#[test]
fn plaintext_secret_rejected_every_layer() {
    let registry = fixture_registry(1);
    for (index, kind) in [
        ConfigSourceKind::File,
        ConfigSourceKind::Environment,
        ConfigSourceKind::Cli,
    ]
    .into_iter()
    .enumerate()
    {
        let candidate = document(
            kind,
            u8::try_from(index + 1).expect("small"),
            [(
                path("secrets", "qdrant_api_secret_ref"),
                LayerOperation::Set(DocumentValue::Text("actual-api-key".into())),
            )],
        );
        let layers = ConfigLayers {
            defaults: source(ConfigSourceKind::Defaults, 0),
            requested_profile: ProfileId::new("direct").expect("profile"),
            file: (kind == ConfigSourceKind::File).then_some(candidate.clone()),
            environment: (kind == ConfigSourceKind::Environment).then_some(candidate.clone()),
            cli: (kind == ConfigSourceKind::Cli).then_some(candidate),
        };
        assert_eq!(
            merge_layers(layers, &registry, limits()),
            Err(ConfigError::SecretPlaintextForbidden),
            "source {kind:?}"
        );
    }
}

#[test]
fn package_section_collision_rejected() {
    let descriptor = section(
        "instance",
        "search-runtime-owner",
        1,
        ReloadClass::DrainAndRestart,
        SecretPolicy::ForbidPlaintext,
        1,
        vec![field(
            "mode",
            ConfigValueKind::Text,
            ConfigValue::Text("standalone".into()),
            &[ConfigSourceKind::File],
            false,
            SecurityFloor::None,
            search_config::RedactionPolicy::Public,
            &[ReconfigurationAction::DrainAndRestart],
        )],
    );
    assert_eq!(
        register_sections(1, [descriptor.clone(), descriptor], limits()),
        Err(ConfigError::SectionConflict)
    );
}

#[test]
fn mixed_reconfiguration_obligations_preserved() {
    let registry = fixture_registry(1);
    let old = snapshot(&registry, None, None, None, "direct", "direct", 30);
    let candidate = snapshot(
        &registry,
        Some(document(
            ConfigSourceKind::File,
            1,
            [
                (
                    path("source_admission", "allow_generated"),
                    LayerOperation::Set(DocumentValue::Boolean(false)),
                ),
                (
                    path("lexical", "profile_id"),
                    LayerOperation::Set(DocumentValue::Text("new-profile-v2".into())),
                ),
            ],
        )),
        None,
        None,
        "direct",
        "direct",
        40,
    );
    let delta = diff(&old, &candidate, &registry).expect("delta");
    let plan = plan_reconfiguration(&delta).expect("plan");
    assert!(
        plan.required_actions
            .contains(&ReconfigurationAction::SecurityBarrier)
    );
    assert!(
        plan.required_actions
            .contains(&ReconfigurationAction::NewCollectionGeneration)
    );
    assert!(
        plan.required_actions
            .contains(&ReconfigurationAction::RebuildProjection)
    );
    assert!(
        delta
            .sections
            .get(&name("source_admission"))
            .expect("section")
            .restrictive
    );
}

#[test]
fn redacted_view_never_discloses_secret_or_absolute_path() {
    let registry = fixture_registry(1);
    let candidate = snapshot(
        &registry,
        Some(document(
            ConfigSourceKind::File,
            1,
            [
                (
                    path("instance", "data_root"),
                    LayerOperation::Set(DocumentValue::Text(
                        "C:/Users/alice/private-search-data".into(),
                    )),
                ),
                (
                    path("secrets", "qdrant_api_secret_ref"),
                    LayerOperation::Set(DocumentValue::Text(
                        "secret://credential/qdrant-production".into(),
                    )),
                ),
            ],
        )),
        None,
        None,
        "direct",
        "direct",
        50,
    );
    let view = redacted_view(&candidate, &registry, DisclosureLevel::Ordinary, limits());
    let rendered = format!("{view:?}");
    assert!(!rendered.contains("alice"));
    assert!(!rendered.contains("qdrant-production"));
    assert!(matches!(
        view.entries.get(&path("instance", "data_root")),
        Some(RedactedValue::PathDigest {
            class: PathLocationClass::AbsoluteLocal,
            ..
        })
    ));
    assert_eq!(
        view.entries.get(&path("secrets", "qdrant_api_secret_ref")),
        Some(&RedactedValue::SecretHidden)
    );
}

#[test]
fn fixed_security_floor_cannot_be_weakened() {
    let registry = fixture_registry(1);
    let candidate = document(
        ConfigSourceKind::File,
        1,
        [(
            path("control", "durability"),
            LayerOperation::Set(DocumentValue::Text("best_effort".into())),
        )],
    );
    assert_eq!(
        merge_layers(
            ConfigLayers {
                defaults: source(ConfigSourceKind::Defaults, 0),
                requested_profile: ProfileId::new("direct").expect("profile"),
                file: Some(candidate),
                environment: None,
                cli: None,
            },
            &registry,
            limits(),
        ),
        Err(ConfigError::SecurityFloorViolation)
    );
}

#[test]
fn reset_is_explicit_and_restores_compiled_default() {
    let registry = fixture_registry(1);
    let file = document(
        ConfigSourceKind::File,
        1,
        [(
            path("instance", "mode"),
            LayerOperation::Set(DocumentValue::Text("managed_client".into())),
        )],
    );
    let cli = document(
        ConfigSourceKind::Cli,
        2,
        [(path("instance", "mode"), LayerOperation::Reset)],
    );
    let merged = merge_layers(
        ConfigLayers {
            defaults: source(ConfigSourceKind::Defaults, 0),
            requested_profile: ProfileId::new("direct").expect("profile"),
            file: Some(file),
            environment: None,
            cli: Some(cli),
        },
        &registry,
        limits(),
    )
    .expect("merge");
    let value = merged.field(&path("instance", "mode")).expect("field");
    assert_eq!(value.value, ConfigValue::Text("standalone".into()));
    assert!(value.provenance.explicit_reset);
    assert_eq!(value.provenance.source.kind, ConfigSourceKind::Cli);
}

#[test]
fn selected_profile_is_external_authority_not_file_self_authorization() {
    let registry = fixture_registry(1);
    let merged = merge_layers(
        ConfigLayers {
            defaults: source(ConfigSourceKind::Defaults, 0),
            requested_profile: ProfileId::new("semantic_optional").expect("profile"),
            file: Some(
                ConfigDocument::from_entries(
                    1,
                    Some(ProfileId::new("semantic_optional").expect("profile")),
                    source(ConfigSourceKind::File, 1),
                    [],
                    limits(),
                )
                .expect("document"),
            ),
            environment: None,
            cli: None,
        },
        &registry,
        limits(),
    )
    .expect("requested profile may be parsed");
    let validated = registry
        .sections()
        .enumerate()
        .map(|(index, (_, descriptor))| {
            ValidatedSection::new(
                project_section(&merged, descriptor).expect("project"),
                ProfileId::new("semantic_optional").expect("profile"),
                Blake3Digest32::from_bytes([u8::try_from(index).expect("small"); 32]),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        assemble_effective(
            &registry,
            validated,
            ProfileId::new("direct").expect("externally selected profile"),
            limits(),
        ),
        Err(ConfigError::ProfileNotAuthorized)
    );
}

#[test]
fn stale_descriptor_rejects_complete_candidate() {
    let original = fixture_registry(1);
    let merged = merge_layers(
        ConfigLayers {
            defaults: source(ConfigSourceKind::Defaults, 0),
            requested_profile: ProfileId::new("direct").expect("profile"),
            file: None,
            environment: None,
            cli: None,
        },
        &original,
        limits(),
    )
    .expect("merge");
    let validated = original
        .sections()
        .enumerate()
        .map(|(index, (_, descriptor))| {
            ValidatedSection::new(
                project_section(&merged, descriptor).expect("project"),
                ProfileId::new("direct").expect("profile"),
                Blake3Digest32::from_bytes([u8::try_from(index).expect("small"); 32]),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        assemble_effective(
            &fixture_registry(2),
            validated,
            ProfileId::new("direct").expect("profile"),
            limits(),
        ),
        Err(ConfigError::StaleDescriptor)
    );
}

#[test]
fn profile_change_requires_gate_and_preserves_all_affected_owners() {
    let registry = fixture_registry(1);
    let old = snapshot(&registry, None, None, None, "direct", "direct", 60);
    let candidate = snapshot(&registry, None, None, None, "lexical", "lexical", 70);
    let delta = diff(&old, &candidate, &registry).expect("delta");
    let plan = plan_reconfiguration(&delta).expect("plan");
    assert!(plan.activation_blocked);
    assert!(
        plan.required_actions
            .contains(&ReconfigurationAction::GateRequired)
    );
    assert_eq!(plan.affected_capabilities.len(), registry.len());
}

#[test]
fn unknown_prefixed_environment_variable_is_error() {
    let registry = fixture_registry(1);
    assert_eq!(
        validate_environment_key("ELIOT_SEARCH__INSTANCE__TYPO", &registry),
        Err(ConfigError::InvalidEnvironmentKey)
    );
    assert!(validate_environment_key("ELIOT_SEARCH__INSTANCE__DATA_ROOT", &registry).is_ok());
}

#[test]
fn debug_projections_do_not_dump_unvalidated_text() {
    let raw = DocumentValue::Text("candidate-secret-or-path".into());
    assert!(!format!("{raw:?}").contains("candidate-secret-or-path"));
    let typed = ConfigValue::Text("C:/private/path".into());
    assert!(!format!("{typed:?}").contains("C:/private/path"));
}

#[test]
fn noop_delta_has_no_steps_or_receipts() {
    let registry = fixture_registry(1);
    let snapshot = snapshot(&registry, None, None, None, "direct", "direct", 80);
    let delta = diff(&snapshot, &snapshot, &registry).expect("delta");
    assert!(delta.is_empty());
    let plan = plan_reconfiguration(&delta).expect("plan");
    assert!(plan.is_noop());
    assert!(plan.ordered_steps.is_empty());
    assert!(plan.required_receipts.is_empty());
    assert_eq!(plan.affected_capabilities, BTreeSet::new());
}
