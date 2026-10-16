//! #586 pure owner-validation fixtures; no backend resolution or runtime application.
use super::*;
use search_config as cfg;
use search_contracts::{Blake3ByteBudget, ProfileId, blake3_bounded_bytes};
use std::num::NonZeroU64;

const LIMITS: cfg::ConfigLimits = cfg::ConfigLimits::W1;
const REFERENCE: &str = "qdrant_api_secret_ref";
const CANARY: &str = "secret://credential/never-disclose-586";

fn profile() -> ProfileId {
    ProfileId::new("direct").unwrap()
}
fn key(name: &str) -> cfg::ConfigKeyName {
    cfg::ConfigKeyName::new(name, 128).unwrap()
}
fn path(name: &str) -> cfg::ConfigKeyPath {
    cfg::ConfigKeyPath::new(
        cfg::ConfigSectionName::new("secrets", 128).unwrap(),
        key(name),
    )
}
fn source(kind: cfg::ConfigSourceKind, tag: u8) -> cfg::ConfigSource {
    cfg::ConfigSource {
        kind,
        source_ref: cfg::ConfigSourceRef::new("fixture", 128).unwrap(),
        source_digest: blake3_bounded_bytes(&[tag], Blake3ByteBudget::new(1).unwrap()).unwrap(),
    }
}
fn layer(
    kind: cfg::ConfigSourceKind,
    entries: Vec<(&str, cfg::LayerOperation)>,
) -> Result<cfg::ConfigDocument, cfg::ConfigError> {
    cfg::ConfigDocument::from_entries(
        2,
        (kind == cfg::ConfigSourceKind::File).then(profile),
        source(kind, 1),
        entries.into_iter().map(|(name, value)| (path(name), value)),
        LIMITS,
    )
}
fn project(
    descriptor: &cfg::ConfigSectionDescriptor,
    layers: [Option<cfg::ConfigDocument>; 3],
) -> Result<cfg::ValidatedSection, cfg::ConfigError> {
    let registry = cfg::register_sections(2, [descriptor.clone()], LIMITS)?;
    let [file, environment, cli] = layers;
    let merged = cfg::merge_layers(
        cfg::ConfigLayers {
            defaults: source(cfg::ConfigSourceKind::Defaults, 0),
            requested_profile: profile(),
            file,
            environment,
            cli,
        },
        &registry,
        LIMITS,
    )?;
    cfg::ValidatedSection::new(
        cfg::project_section(&merged, descriptor)?,
        profile(),
        LIMITS,
    )
}
fn input(
    kind: cfg::ConfigSourceKind,
    entries: Vec<(&str, cfg::LayerOperation)>,
) -> Result<cfg::ValidatedSection, cfg::ConfigError> {
    let mut layers = [None, None, None];
    if !entries.is_empty() {
        let index = match kind {
            cfg::ConfigSourceKind::File => 0,
            cfg::ConfigSourceKind::Environment => 1,
            cfg::ConfigSourceKind::Cli => 2,
            cfg::ConfigSourceKind::Defaults => unreachable!(),
        };
        layers[index] = Some(layer(kind, entries)?);
    }
    project(&section_descriptor()?, layers)
}
fn set(value: cfg::DocumentValue) -> cfg::LayerOperation {
    cfg::LayerOperation::Set(value)
}
fn text(value: &str) -> cfg::LayerOperation {
    set(cfg::DocumentValue::Text(value.into()))
}
fn file(
    name: &str,
    value: cfg::LayerOperation,
) -> Result<ValidatedSecretsConfig, cfg::ConfigError> {
    validate_secrets_section(&input(cfg::ConfigSourceKind::File, vec![(name, value)])?)
}
fn defaults() -> cfg::ValidatedSection {
    input(cfg::ConfigSourceKind::Defaults, vec![]).unwrap()
}
fn snapshot(section: &cfg::ValidatedSection) -> cfg::EffectiveConfigSnapshot {
    validate_secrets_section(section).unwrap();
    let registry = cfg::register_sections(2, [section_descriptor().unwrap()], LIMITS).unwrap();
    cfg::assemble_effective(&registry, [section.clone()], profile(), LIMITS).unwrap()
}

#[test]
fn documented_defaults_exact_inventory_and_descriptor_revision() {
    let descriptor = section_descriptor().unwrap();
    assert_eq!(descriptor.schema_revision().get(), 2);
    assert_eq!(descriptor.owner().as_str(), "search-os-secrets");
    assert_eq!(
        descriptor.minimum_reload(),
        cfg::ReloadClass::RestartDependency
    );
    assert_eq!(
        descriptor
            .fields()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        ["backend", "lease_ttl_ms", REFERENCE, "rotation_grace_ms"]
    );
    let section = defaults();
    let checked = validate_secrets_section(&section).unwrap();
    assert_eq!(
        (checked.lease_ttl_ms(), checked.rotation_grace_ms()),
        (30_000, 60_000)
    );
    assert!(checked.qdrant_api_secret_ref().is_none());
    assert_eq!(
        checked.backend(),
        file("backend", text("windows_user_bound"))
            .unwrap()
            .backend()
    );
    assert_eq!(
        section.field(&key("backend")).unwrap().value,
        cfg::ConfigValue::Text("windows_user_bound".into())
    );
    assert_eq!(snapshot(&section).config_schema_version(), 2);
    assert_eq!(snapshot(&section).selected_profile().as_str(), "direct");
    let raw = cfg::parse_document(b"schema_version=2\nprofile=\"direct\"\n[secrets]\nbackend=\"windows_user_bound\"\nlease_ttl_ms=30000\nrotation_grace_ms=60000\nqdrant_api_secret_ref=\"secret://credential/raw-direct-586\"\n",
        source(cfg::ConfigSourceKind::File, 1), LIMITS).unwrap();
    let rebuilt = project(&descriptor, [Some(raw), None, None]).unwrap();
    assert!(
        validate_secrets_section(&rebuilt)
            .unwrap()
            .qdrant_api_secret_ref()
            .is_some()
    );
}

#[test]
fn documented_numeric_bounds_are_inclusive_without_new_limits() {
    for (name, number, accepted) in [
        ("lease_ttl_ms", 999, false),
        ("lease_ttl_ms", 1_000, true),
        ("lease_ttl_ms", 300_000, true),
        ("lease_ttl_ms", 300_001, false),
        ("rotation_grace_ms", -1, false),
        ("rotation_grace_ms", 0, true),
        ("rotation_grace_ms", 600_000, true),
        ("rotation_grace_ms", 600_001, false),
    ] {
        let result = file(name, set(cfg::DocumentValue::Integer(number)));
        assert_eq!(result.is_ok(), accepted, "{name}={number}");
    }
}

#[test]
fn enum_unknown_fields_types_and_nonreference_values_fail_closed() {
    for (name, value) in [
        ("backend", cfg::DocumentValue::Text("memory".into())),
        ("backend", cfg::DocumentValue::Boolean(true)),
        ("lease_ttl_ms", cfg::DocumentValue::Text("30000".into())),
        ("rotation_grace_ms", cfg::DocumentValue::Boolean(false)),
        (REFERENCE, cfg::DocumentValue::Integer(1)),
        ("unknown", cfg::DocumentValue::Integer(1)),
        (
            REFERENCE,
            cfg::DocumentValue::Text("plaintext-586-canary".into()),
        ),
        (REFERENCE, cfg::DocumentValue::Text("secret://".into())),
        (
            REFERENCE,
            cfg::DocumentValue::Text("secret://has whitespace".into()),
        ),
    ] {
        assert!(file(name, set(value)).is_err(), "{name}");
    }
    let registry = cfg::register_sections(2, [section_descriptor().unwrap()], LIMITS).unwrap();
    let document = cfg::parse_document(
        b"schema_version = 2\nprofile = \"direct\"\n[foreign]\nvalue = 1\n",
        source(cfg::ConfigSourceKind::File, 1),
        LIMITS,
    )
    .unwrap();
    assert!(matches!(
        cfg::merge_layers(
            cfg::ConfigLayers {
                defaults: source(cfg::ConfigSourceKind::Defaults, 0),
                requested_profile: profile(),
                file: Some(document),
                environment: None,
                cli: None,
            },
            &registry,
            LIMITS
        ),
        Err(cfg::ConfigError::UnknownSection)
    ));
}

#[test]
fn file_environment_cli_sources_and_whitelist_remain_closed() {
    let registry = cfg::register_sections(2, [section_descriptor().unwrap()], LIMITS).unwrap();
    for (name, value) in [
        ("backend", text("windows_user_bound")),
        ("lease_ttl_ms", set(cfg::DocumentValue::Integer(20_000))),
        (
            "rotation_grace_ms",
            set(cfg::DocumentValue::Integer(10_000)),
        ),
        (REFERENCE, text(CANARY)),
    ] {
        assert!(file(name, value.clone()).is_ok(), "{name}");
        for kind in [
            cfg::ConfigSourceKind::Environment,
            cfg::ConfigSourceKind::Cli,
        ] {
            let result = input(kind, vec![(name, value.clone())])
                .and_then(|section| validate_secrets_section(&section));
            assert_eq!(
                result.is_ok(),
                name == REFERENCE && kind == cfg::ConfigSourceKind::Environment
            );
        }
    }
    assert_eq!(
        cfg::validate_environment_key("ELIOT_SEARCH__SECRETS__QDRANT_API_SECRET_REF", &registry)
            .unwrap(),
        path(REFERENCE)
    );
    for suffix in [
        "BACKEND",
        "LEASE_TTL_MS",
        "ROTATION_GRACE_MS",
        "UNKNOWN",
        "qdrant_api_secret_ref",
    ] {
        assert!(
            cfg::validate_environment_key(&format!("ELIOT_SEARCH__SECRETS__{suffix}"), &registry)
                .is_err()
        );
    }
    assert!(
        file(REFERENCE, text(CANARY))
            .unwrap()
            .qdrant_api_secret_ref()
            .is_some()
    );
    // Generic typed construction alone must not mint owner-approved CLI provenance.
    let descriptor = section_descriptor().unwrap();
    let permissive = cfg::ConfigSectionDescriptor::new(
        descriptor.name().clone(),
        descriptor.owner().clone(),
        descriptor.schema_revision(),
        descriptor.minimum_reload(),
        descriptor.field_registry_digest(),
        descriptor.secret_policy(),
        descriptor.fields().map(|(_, field)| {
            cfg::ConfigFieldDescriptor::new(
                field.key().clone(),
                field.kind(),
                field.default().clone(),
                field.bounds(),
                [
                    cfg::ConfigSourceKind::File,
                    cfg::ConfigSourceKind::Environment,
                    cfg::ConfigSourceKind::Cli,
                ],
                field.reset_allowed(),
                field.security_floor(),
                field.redaction(),
                field.actions(),
            )
            .unwrap()
        }),
        LIMITS,
    )
    .unwrap();
    let cli = layer(cfg::ConfigSourceKind::Cli, vec![(REFERENCE, text(CANARY))]).unwrap();
    let untrusted = project(&permissive, [None, None, Some(cli)]).unwrap();
    assert!(matches!(
        validate_secrets_section(&untrusted),
        Err(cfg::ConfigError::OverrideNotAllowed)
    ));
}

#[test]
fn reset_restores_reference_absence_but_cannot_bypass_source_permissions() {
    for name in ["backend", "lease_ttl_ms", "rotation_grace_ms"] {
        assert!(
            !section_descriptor()
                .unwrap()
                .field(&key(name))
                .unwrap()
                .reset_allowed()
        );
        assert!(matches!(
            file(name, cfg::LayerOperation::Reset),
            Err(cfg::ConfigError::ResetNotAllowed)
        ));
    }
    let descriptor = section_descriptor().unwrap();
    assert!(descriptor.field(&key(REFERENCE)).unwrap().reset_allowed());
    let original = layer(cfg::ConfigSourceKind::File, vec![(REFERENCE, text(CANARY))]).unwrap();
    let reset = layer(
        cfg::ConfigSourceKind::Environment,
        vec![(REFERENCE, cfg::LayerOperation::Reset)],
    )
    .unwrap();
    let section = project(&descriptor, [Some(original), Some(reset), None]).unwrap();
    assert!(
        validate_secrets_section(&section)
            .unwrap()
            .qdrant_api_secret_ref()
            .is_none()
    );
    assert!(
        section
            .field(&key(REFERENCE))
            .unwrap()
            .provenance
            .explicit_reset
    );
    assert!(
        file(REFERENCE, cfg::LayerOperation::Reset)
            .unwrap()
            .qdrant_api_secret_ref()
            .is_none()
    );
    assert!(matches!(
        input(
            cfg::ConfigSourceKind::Cli,
            vec![(REFERENCE, cfg::LayerOperation::Reset)]
        ),
        Err(cfg::ConfigError::OverrideNotAllowed)
    ));
    assert_ne!(
        snapshot(&section).fingerprint(),
        snapshot(&defaults()).fingerprint()
    );
}

#[test]
fn canonical_v2_binds_values_provenance_and_refuses_partial_revision_one() {
    let section = defaults();
    let checked = validate_secrets_section(&section).unwrap();
    assert_eq!(section_digest(&checked), section.validated_section_digest());
    assert_eq!(
        section_digest(&checked),
        section_digest(&validate_secrets_section(&defaults()).unwrap())
    );
    let fingerprint = snapshot(&section).fingerprint();
    assert_eq!(
        (
            fingerprint.algorithm(),
            fingerprint.canonical_profile(),
            fingerprint.revision()
        ),
        ("blake3", "eliot/cbor/config-snapshot/v2", 2)
    );
    assert_ne!(
        fingerprint,
        cfg::ConfigFingerprint::restore_legacy_v1(*fingerprint.as_bytes())
    );
    for (name, value) in [
        ("backend", text("windows_user_bound")),
        ("lease_ttl_ms", set(cfg::DocumentValue::Integer(20_000))),
        (
            "rotation_grace_ms",
            set(cfg::DocumentValue::Integer(10_000)),
        ),
        (REFERENCE, text(CANARY)),
    ] {
        let changed = input(cfg::ConfigSourceKind::File, vec![(name, value)]).unwrap();
        assert_ne!(
            section_digest(&validate_secrets_section(&changed).unwrap()),
            section_digest(&checked)
        );
        assert_ne!(snapshot(&changed).fingerprint(), fingerprint);
    }
    // Historical daemon v1 partial registry: preserve its exact preimage, not a v2 relabel.
    let descriptor = section_descriptor().unwrap();
    let mut preimage = b"eliot-searchd/section-registry/v1\0secrets\0search-os-secrets\0".to_vec();
    preimage.extend_from_slice(&1_u64.to_be_bytes());
    preimage.extend_from_slice(b"qdrant_api_secret_ref\0secret\0");
    let digest =
        blake3_bounded_bytes(&preimage, Blake3ByteBudget::new(preimage.len()).unwrap()).unwrap();
    let historical = cfg::ConfigSectionDescriptor::new(
        descriptor.name().clone(),
        descriptor.owner().clone(),
        NonZeroU64::MIN,
        descriptor.minimum_reload(),
        digest,
        descriptor.secret_policy(),
        [descriptor.field(&key(REFERENCE)).unwrap().clone()],
        LIMITS,
    )
    .unwrap();
    let stale = project(&historical, [None, None, None]).unwrap();
    assert!(matches!(
        validate_secrets_section(&stale),
        Err(cfg::ConfigError::StaleDescriptor)
    ));
}

#[test]
fn ordinary_privileged_and_owner_debug_never_resolve_or_disclose_references() {
    let section = input(cfg::ConfigSourceKind::File, vec![(REFERENCE, text(CANARY))]).unwrap();
    let checked = validate_secrets_section(&section).unwrap();
    let snapshot = snapshot(&section);
    let registry = cfg::register_sections(2, [section_descriptor().unwrap()], LIMITS).unwrap();
    for disclosure in [
        cfg::DisclosureLevel::Ordinary,
        cfg::DisclosureLevel::PrivilegedMetadata,
    ] {
        let view = cfg::redacted_view(&snapshot, &registry, disclosure, LIMITS).unwrap();
        assert!(matches!(
            view.entries.get(&path(REFERENCE)),
            Some(cfg::RedactedValue::SecretHidden)
        ));
        assert!(!format!("{view:?} {checked:?} {section:?}").contains("never-disclose-586"));
    }
    let error = file(REFERENCE, text("plaintext-586-canary")).err().unwrap();
    assert!(!format!("{error:?} {error}").contains("plaintext-586-canary"));
}

#[test]
fn reload_direction_keeps_exact_composite_actions_and_section_floor() {
    use cfg::ReconfigurationAction::{ApplyLive, RestartDependency};
    use std::collections::BTreeSet;
    let baseline = validate_secrets_section(&defaults()).unwrap();
    for (name, value, expected) in [
        (
            "lease_ttl_ms",
            20_000,
            BTreeSet::from([ApplyLive, RestartDependency]),
        ),
        ("lease_ttl_ms", 40_000, BTreeSet::from([RestartDependency])),
        (
            "rotation_grace_ms",
            10_000,
            BTreeSet::from([ApplyLive, RestartDependency]),
        ),
        (
            "rotation_grace_ms",
            70_000,
            BTreeSet::from([ApplyLive, RestartDependency]),
        ),
    ] {
        let changed = file(name, set(cfg::DocumentValue::Integer(value))).unwrap();
        let decision = plan_section_change(&baseline, &changed).unwrap();
        assert_eq!(decision.required_actions, expected, "{name}={value}");
        assert_eq!(decision.changed_key_paths, BTreeSet::from([path(name)]));
    }
    let changed = file(REFERENCE, text(CANARY)).unwrap();
    let decision = plan_section_change(&baseline, &changed).unwrap();
    assert_eq!(
        decision.required_actions,
        BTreeSet::from([RestartDependency])
    );
    assert_eq!(
        decision.changed_key_paths,
        BTreeSet::from([path(REFERENCE)])
    );
    let unchanged = plan_section_change(&baseline, &baseline).unwrap();
    assert!(unchanged.changed_key_paths.is_empty() && unchanged.required_actions.is_empty());
    let restated = file("backend", text("windows_user_bound")).unwrap();
    let unchanged = plan_section_change(&baseline, &restated).unwrap();
    assert!(unchanged.changed_key_paths.is_empty() && unchanged.required_actions.is_empty());
    let changed_section = input(
        cfg::ConfigSourceKind::File,
        vec![
            ("lease_ttl_ms", set(cfg::DocumentValue::Integer(20_000))),
            (REFERENCE, text(CANARY)),
        ],
    )
    .unwrap();
    let changed = validate_secrets_section(&changed_section).unwrap();
    let decision = plan_section_change(&baseline, &changed).unwrap();
    assert_eq!(
        decision.required_actions,
        BTreeSet::from([ApplyLive, RestartDependency])
    );
    assert_eq!(
        decision.changed_key_paths,
        BTreeSet::from([path("lease_ttl_ms"), path(REFERENCE)])
    );
}
