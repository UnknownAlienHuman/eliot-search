//! Focused owner-descriptor cutover fixtures; no runtime or secret resolution.
use super::super::{
    capture_file_document,
    spec::{
        blake3_digest, const_bounds, daemon_limits, defaults_source, key_name, owner_name,
        profile_id, section_name,
    },
};
use super::{build_effective, build_effective_defaults, daemon_registry};
use search_config::{
    ConfigError, ConfigFieldDescriptor, ConfigLayers, ConfigSectionDescriptor, ConfigSourceKind,
    ConfigValue, ConfigValueKind, ReconfigurationAction, RedactionPolicy, ReloadClass,
    SecretPolicy, SecurityFloor, ValidatedSection, assemble_effective, merge_layers,
    project_section, register_sections,
};
use std::num::NonZeroU64;

#[test]
fn secrets_owner_settings_are_retained_with_revision_two_defaults_and_restart_floor() {
    let effective = build_effective_defaults().unwrap();
    let name = section_name("secrets").unwrap();
    let descriptor = effective.registry().section(&name).unwrap();
    assert_eq!(
        descriptor,
        &search_os_secrets::section_descriptor().unwrap()
    );
    assert_eq!(descriptor.schema_revision().get(), 2);
    assert_eq!(descriptor.minimum_reload(), ReloadClass::RestartDependency);
    let section = effective.snapshot().section(&name).unwrap().validated();
    assert_eq!(section.fields().len(), 4);
    assert_eq!(
        effective.secrets_settings(),
        &search_os_secrets::validate_secrets_section(section).unwrap()
    );
    for (name, expected) in [
        ("backend", ConfigValue::Text("windows_user_bound".into())),
        ("lease_ttl_ms", ConfigValue::Integer(30_000)),
        ("rotation_grace_ms", ConfigValue::Integer(60_000)),
        ("qdrant_api_secret_ref", ConfigValue::Absent),
    ] {
        assert_eq!(
            section.field(&key_name(name).unwrap()).unwrap().value,
            expected
        );
    }
    assert_eq!(effective, effective.clone());
}

#[test]
fn selected_handles_config_comes_from_owner_and_marks_restrictive_changes() {
    let defaults = build_effective_defaults().unwrap();
    let handles_name = section_name("handles").unwrap();
    let descriptor = defaults.registry().section(&handles_name).unwrap();
    assert_eq!(descriptor, &search_handles::section_descriptor().unwrap());
    assert_eq!(descriptor.schema_revision().get(), 1);
    assert_eq!(descriptor.minimum_reload(), ReloadClass::ApplyLive);

    let defaults_section = defaults
        .snapshot()
        .section(&handles_name)
        .unwrap()
        .validated();
    assert_eq!(defaults_section.fields().len(), 5);
    assert_eq!(
        defaults.handles_settings(),
        &search_handles::validate_handles_section(defaults_section).unwrap()
    );
    assert_eq!(
        (
            defaults.handles_settings().ephemeral_ttl_ms(),
            defaults.handles_settings().max_ephemeral_per_binding(),
            defaults.handles_settings().max_durable_per_binding(),
            defaults.handles_settings().max_expansion_bytes(),
            defaults.handles_settings().durable_unsaved_allowed(),
        ),
        (600_000, 256, 128, 1_048_576, false)
    );

    let file = capture_file_document(
        b"schema_version = 2\nprofile = \"direct\"\n[handles]\nephemeral_ttl_ms = 300000\nmax_ephemeral_per_binding = 64\nmax_durable_per_binding = 32\nmax_expansion_bytes = 524288\ndurable_unsaved_allowed = false\n",
        "handles-selection",
    )
    .unwrap();
    let selected = build_effective(Some(file), None, None, "direct", "direct").unwrap();
    let selected_section = selected
        .snapshot()
        .section(&handles_name)
        .unwrap()
        .validated();
    assert_eq!(
        selected.handles_settings(),
        &search_handles::validate_handles_section(selected_section).unwrap()
    );
    assert_eq!(
        (
            selected.handles_settings().ephemeral_ttl_ms(),
            selected.handles_settings().max_ephemeral_per_binding(),
            selected.handles_settings().max_durable_per_binding(),
            selected.handles_settings().max_expansion_bytes(),
            selected.handles_settings().durable_unsaved_allowed(),
        ),
        (300_000, 64, 32, 524_288, false)
    );

    let delta = search_config::diff(
        defaults.snapshot(),
        selected.snapshot(),
        defaults.registry(),
    )
    .unwrap();
    let handles_delta = delta.sections.get(&handles_name).unwrap();
    assert!(handles_delta.restrictive);
    assert!(
        handles_delta
            .required_actions
            .contains(&ReconfigurationAction::ApplyLive)
    );
}

#[test]
fn handles_config_rejects_invalid_bounds_and_non_file_sources() {
    let zero_ttl = capture_file_document(
        b"schema_version = 2\nprofile = \"direct\"\n[handles]\nephemeral_ttl_ms = 0\n",
        "handles-zero-ttl",
    )
    .unwrap();
    assert!(build_effective(Some(zero_ttl), None, None, "direct", "direct").is_err());

    let minimum_ttl = capture_file_document(
        b"schema_version = 2\nprofile = \"direct\"\n[handles]\nephemeral_ttl_ms = 1\n",
        "handles-minimum-ttl",
    )
    .unwrap();
    assert!(build_effective(Some(minimum_ttl), None, None, "direct", "direct").is_ok());

    let maximum_ttl = capture_file_document(
        b"schema_version = 2\nprofile = \"direct\"\n[handles]\nephemeral_ttl_ms = 16777216\n",
        "handles-maximum-ttl",
    )
    .unwrap();
    assert!(build_effective(Some(maximum_ttl), None, None, "direct", "direct").is_ok());

    let over_bound = capture_file_document(
        b"schema_version = 2\nprofile = \"direct\"\n[handles]\nephemeral_ttl_ms = 16777217\n",
        "handles-over-bound",
    )
    .unwrap();
    assert!(build_effective(Some(over_bound), None, None, "direct", "direct").is_err());

    let unsaved_enabled = capture_file_document(
        b"schema_version = 2\nprofile = \"direct\"\n[handles]\ndurable_unsaved_allowed = true\n",
        "handles-unsaved-enabled",
    )
    .unwrap();
    assert!(build_effective(Some(unsaved_enabled), None, None, "direct", "direct").is_err());

    assert!(
        super::super::capture_environment_document(&[(
            "ELIOT_SEARCH__HANDLES__EPHEMERAL_TTL_MS",
            "300000",
        )])
        .is_err()
    );

    let cli = super::super::capture_cli_document(&[("handles.ephemeral_ttl_ms", "300000")])
        .unwrap()
        .unwrap();
    assert!(build_effective(None, None, Some(cli), "direct", "direct").is_err());
}

#[test]
fn old_raw_opaque_ref_document_gets_owner_defaults_without_disclosing_reference() {
    let file = capture_file_document(b"schema_version = 2\nprofile = \"direct\"\n[secrets]\nqdrant_api_secret_ref = \"secret://credential/owner-cutover-canary\"\n", "fixture").unwrap();
    let effective = build_effective(Some(file), None, None, "direct", "direct").unwrap();
    let name = section_name("secrets").unwrap();
    let section = effective.snapshot().section(&name).unwrap().validated();
    assert_eq!(section.schema_revision().get(), 2);
    assert_eq!(section.fields().len(), 4);
    assert!(matches!(
        section
            .field(&key_name("qdrant_api_secret_ref").unwrap())
            .unwrap()
            .value,
        ConfigValue::SecretReference(_)
    ));
    assert_eq!(
        section
            .field(&key_name("lease_ttl_ms").unwrap())
            .unwrap()
            .value,
        ConfigValue::Integer(30_000)
    );
    assert!(!format!("{:?}", effective.secrets_settings()).contains("owner-cutover-canary"));
    assert_eq!(
        effective.secrets_settings(),
        &search_os_secrets::validate_secrets_section(section).unwrap()
    );
}

#[test]
fn invalid_owner_backend_and_numeric_edges_never_return_effective_config() {
    for field in [
        "backend = \"other\"",
        "lease_ttl_ms = 999",
        "lease_ttl_ms = 300001",
        "rotation_grace_ms = -1",
        "rotation_grace_ms = 600001",
    ] {
        let input = format!("schema_version = 2\nprofile = \"direct\"\n[secrets]\n{field}\n");
        let file = capture_file_document(input.as_bytes(), "fixture").unwrap();
        assert!(build_effective(Some(file), None, None, "direct", "direct").is_err());
    }
}

#[test]
fn historical_partial_secrets_revision_one_is_stale_without_relabelling() {
    let limits = daemon_limits();
    // Exact former daemon descriptor identity, retained only as historical input.
    let mut old_identity =
        b"eliot-searchd/section-registry/v1\0secrets\0search-os-secrets\0".to_vec();
    old_identity.extend_from_slice(&1_u64.to_be_bytes());
    old_identity.extend_from_slice(b"qdrant_api_secret_ref\0secret\0");
    let descriptor = ConfigSectionDescriptor::new(
        section_name("secrets").unwrap(),
        owner_name("search-os-secrets").unwrap(),
        NonZeroU64::MIN,
        ReloadClass::RestartDependency,
        blake3_digest(&old_identity),
        SecretPolicy::OpaqueReferencesOnly,
        [ConfigFieldDescriptor::new(
            key_name("qdrant_api_secret_ref").unwrap(),
            ConfigValueKind::SecretReference,
            ConfigValue::Absent,
            const_bounds(),
            [ConfigSourceKind::File, ConfigSourceKind::Environment],
            true,
            SecurityFloor::None,
            RedactionPolicy::Secret,
            [ReconfigurationAction::RestartDependency],
        )
        .unwrap()],
        limits,
    )
    .unwrap();
    let registry = register_sections(2, [descriptor.clone()], limits).unwrap();
    let profile = profile_id("direct").unwrap();
    let merged = merge_layers(
        ConfigLayers {
            defaults: defaults_source().unwrap(),
            requested_profile: profile.clone(),
            file: None,
            environment: None,
            cli: None,
        },
        &registry,
        limits,
    )
    .unwrap();
    let historical = ValidatedSection::new(
        project_section(&merged, &descriptor).unwrap(),
        profile.clone(),
        limits,
    )
    .unwrap();
    assert_eq!(historical.schema_revision().get(), 1);
    assert_eq!(
        search_os_secrets::validate_secrets_section(&historical),
        Err(ConfigError::StaleDescriptor)
    );
    assert_eq!(
        assemble_effective(
            &daemon_registry().unwrap(),
            [historical.clone()],
            profile,
            limits
        ),
        Err(ConfigError::StaleDescriptor)
    );
    assert_eq!(historical.schema_revision().get(), 1);
}
