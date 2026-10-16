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
