use std::collections::BTreeSet;

use search_config::{
    ConfigDocument, ConfigKeyPath, ConfigSource, ConfigSourceKind,
    ConfigSourceRef, DocumentValue, LayerOperation, ReceiptKind,
};
use search_contracts::Blake3Digest32;

use super::{
    AcceptedReceipts, build_effective, build_effective_defaults,
    capture_cli_document, capture_environment_document, capture_file_document,
    config_status_json, daemon_registry, derive_readiness, direct_dependencies,
    plan_activation, shell_dependencies, try_activate,
};
use crate::sha256;

fn file_doc(text: &str) -> ConfigDocument {
    capture_file_document(text.as_bytes(), "test-file").expect("file parses")
}

fn direct_file(extra: &str) -> ConfigDocument {
    let body = format!("schema_version = 1\nprofile = \"direct\"\n{extra}");
    file_doc(&body)
}

#[test]
fn registry_mirrors_declared_owners_and_reload_classes() {
    let registry = daemon_registry().expect("registry");
    assert_eq!(registry.config_schema_version(), 1);
    assert_eq!(registry.len(), 7);
    let owner = |section: &str| {
        registry
            .section(
                &search_config::ConfigSectionName::new(section, 128)
                    .expect("section"),
            )
            .expect("present")
            .owner()
            .as_str()
            .to_owned()
    };
    assert_eq!(owner("instance"), "search-runtime-owner");
    assert_eq!(owner("secrets"), "search-os-secrets");
    assert_eq!(owner("control"), "search-control-redb");
    assert_eq!(owner("source_admission"), "search-source-admission");
    assert_eq!(owner("lexical"), "search-lexical");
    assert_eq!(owner("query"), "search-query-planner");
    assert_eq!(owner("optional_profiles"), "eliot-searchd");
}

#[test]
fn effective_config_assembles_from_defaults_file_env_cli() {
    let file = direct_file("[query]\nlimit = 20\n");
    let environment =
        capture_environment_document(&[("ELIOT_SEARCH__QUERY__LIMIT", "30")])
            .expect("env")
            .expect("layer");
    let cli = capture_cli_document(&[("query.limit", "40")])
        .expect("cli")
        .expect("layer");
    let effective = build_effective(
        Some(file),
        Some(environment),
        Some(cli),
        "direct",
        "direct",
    )
    .expect("effective");
    let registry = effective.registry();
    let path = ConfigKeyPath::new(
        search_config::ConfigSectionName::new("query", 128).expect("section"),
        search_config::ConfigKeyName::new("limit", 128).expect("key"),
    );
    let descriptor = registry.field(&path).expect("descriptor");
    let (field_descriptor, value) = effective
        .snapshot()
        .field(registry, path.section(), path.key())
        .expect("field");
    assert_eq!(field_descriptor.key().as_str(), descriptor.key().as_str());
    match &value.value {
        search_config::ConfigValue::Integer(number) => assert_eq!(*number, 40),
        other => panic!("CLI must win with 40, got {other:?}"),
    }
    assert_eq!(value.provenance.source.kind, ConfigSourceKind::Cli);
}

#[test]
fn cli_wins_over_environment_over_file_over_defaults() {
    let defaults =
        build_effective(None, None, None, "direct", "direct").expect("defaults");
    let file = build_effective(
        Some(direct_file("[query]\nlimit = 20\n")),
        None,
        None,
        "direct",
        "direct",
    )
    .expect("file");
    assert_ne!(defaults.fingerprint(), file.fingerprint());
    let with_env = build_effective(
        Some(direct_file("[query]\nlimit = 20\n")),
        capture_environment_document(&[("ELIOT_SEARCH__QUERY__LIMIT", "30")])
            .expect("env"),
        None,
        "direct",
        "direct",
    )
    .expect("env wins");
    assert_ne!(file.fingerprint(), with_env.fingerprint());
    let with_cli = build_effective(
        Some(direct_file("[query]\nlimit = 20\n")),
        capture_environment_document(&[("ELIOT_SEARCH__QUERY__LIMIT", "30")])
            .expect("env"),
        capture_cli_document(&[("query.limit", "40")]).expect("cli"),
        "direct",
        "direct",
    )
    .expect("cli wins");
    assert_ne!(with_env.fingerprint(), with_cli.fingerprint());
}

#[test]
fn explicit_reset_restores_compiled_default() {
    let effective = build_effective(
        Some(direct_file("[query]\nlimit = 20\n")),
        None,
        capture_cli_document(&[("query.limit", "__RESET__")]).expect("cli"),
        "direct",
        "direct",
    )
    .expect("reset");
    let section =
        search_config::ConfigSectionName::new("query", 128).expect("section");
    let key = search_config::ConfigKeyName::new("limit", 128).expect("key");
    let field = effective
        .snapshot()
        .section(&section)
        .expect("section")
        .field(&key)
        .expect("field");
    assert_eq!(field.value, search_config::ConfigValue::Integer(10));
    assert!(field.provenance.explicit_reset);
}

#[test]
fn duplicate_unknown_and_wrong_type_fail_closed() {
    assert!(
        capture_file_document(
            b"schema_version = 1\nprofile = \"direct\"\n[query]\nlimit = 1\nlimit = 2\n",
            "dup",
        )
        .is_err()
    );
    assert!(
        build_effective(
            Some(direct_file("[query]\ntypo = 1\n")),
            None,
            None,
            "direct",
            "direct",
        )
        .is_err()
    );
    assert!(
        build_effective(
            Some(direct_file("[query]\nlimit = \"not-a-number\"\n")),
            None,
            None,
            "direct",
            "direct",
        )
        .is_err()
    );
}

#[test]
fn unknown_prefixed_environment_key_fails_closed() {
    assert!(
        capture_environment_document(&[("ELIOT_SEARCH__QUERY__TYPO", "1")])
            .is_err()
    );
    assert!(
        capture_environment_document(&[("ELIOT_SEARCH__QUERY__limit", "1")])
            .is_err()
    );
}

#[test]
fn plaintext_secret_is_denied_on_every_layer() {
    let file =
        direct_file("[secrets]\nqdrant_api_secret_ref = \"actual-api-key\"\n");
    assert!(build_effective(Some(file), None, None, "direct", "direct").is_err());
    let environment = capture_environment_document(&[(
        "ELIOT_SEARCH__SECRETS__QDRANT_API_SECRET_REF",
        "k",
    )]);
    assert!(
        environment.is_err()
            || {
                let layer = environment.expect("checked").expect("layer");
                build_effective(None, Some(layer), None, "direct", "direct")
                    .is_err()
            }
    );
    let cli = capture_cli_document(&[(
        "secrets.qdrant_api_secret_ref",
        "actual-api-key",
    )])
    .expect("cli layer builds");
    assert!(
        build_effective(None, None, cli, "direct", "direct").is_err(),
        "CLI secret plaintext must fail at merge"
    );
}

#[test]
fn opaque_secret_reference_is_accepted() {
    let effective = build_effective(
        Some(direct_file(
            "[secrets]\nqdrant_api_secret_ref = \"secret://credential/qdrant-test\"\n",
        )),
        None,
        None,
        "direct",
        "direct",
    )
    .expect("opaque ref accepted");
    let section =
        search_config::ConfigSectionName::new("secrets", 128).expect("section");
    let key = search_config::ConfigKeyName::new("qdrant_api_secret_ref", 128)
        .expect("key");
    let field = effective
        .snapshot()
        .section(&section)
        .expect("section")
        .field(&key)
        .expect("field");
    assert!(matches!(
        field.value,
        search_config::ConfigValue::SecretReference(_)
    ));
}

#[test]
fn mixed_live_restart_rebuild_failure_retains_old_snapshot() {
    let current = build_effective_defaults().expect("current");
    let candidate = build_effective(
        Some(direct_file(
            "[query]\nlimit = 11\n[source_admission]\nallow_generated = false\n[lexical]\nprofile_id = \"candidate-v2\"\n",
        )),
        None,
        None,
        "direct",
        "direct",
    )
    .expect("candidate");
    assert_ne!(current.fingerprint(), candidate.fingerprint());
    let plan = plan_activation(&current, &candidate).expect("plan");
    assert!(
        plan.required_actions
            .contains(&search_config::ReconfigurationAction::ApplyLive)
    );
    assert!(
        plan.required_actions
            .contains(&search_config::ReconfigurationAction::SecurityBarrier)
    );
    assert!(
        plan.required_actions.contains(
            &search_config::ReconfigurationAction::NewCollectionGeneration
        )
    );
    assert!(
        plan.required_actions
            .contains(&search_config::ReconfigurationAction::RebuildProjection)
    );
    let live_only = BTreeSet::from([ReceiptKind::LiveApply]);
    assert_eq!(
        try_activate(&current, candidate.clone(), &live_only),
        Err(super::ACTIVATION_PARTIAL_REFUSED.to_owned())
    );
    let full = plan
        .required_receipts
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let published =
        try_activate(&current, candidate, &full).expect("full receipts publish");
    assert_ne!(published.fingerprint(), current.fingerprint());
}

#[test]
fn fixed_floor_reject_never_publishes() {
    let candidate = build_effective(
        Some(direct_file(
            "[control]\ndurability = \"best_effort\"\n",
        )),
        None,
        None,
        "direct",
        "direct",
    );
    assert!(candidate.is_err());
}

#[test]
fn w1_readiness_does_not_imply_search_available() {
    let effective = build_effective_defaults().expect("effective");
    let mut dependencies = direct_dependencies();
    dependencies.stores.secret_backend_verified = true;
    let report =
        derive_readiness(&effective, dependencies, AcceptedReceipts::default());
    assert!(report.composition.configuration_ready);
    assert!(report.composition.runtime_owner_ready);
    assert!(report.composition.control_store_ready);
    assert!(report.stores.direct_store_ready);
    assert!(report.capabilities.source_backed_search_available);
    assert!(!report.capabilities.search_available);
    assert!(!report.capabilities.indexed_search_available);
    assert!(report.blockers.contains(&super::SEARCH_NOT_ACCEPTED));
}

#[test]
fn enabling_optional_flag_alone_changes_no_acceptance() {
    let effective = build_effective(
        Some(direct_file(
            "[optional_profiles]\nsemantic = true\n",
        )),
        None,
        None,
        "direct",
        "direct",
    )
    .expect("flag parses");
    let report = derive_readiness(
        &effective,
        direct_dependencies(),
        AcceptedReceipts::default(),
    );
    assert!(!report.capabilities.search_available);
    assert!(report.blockers.contains(&super::OPTIONAL_GATE_REQUIRED));
    let current = build_effective_defaults().expect("current");
    let plan = plan_activation(&current, &effective).expect("plan");
    assert!(plan.activation_blocked);
    assert_eq!(
        try_activate(&current, effective, &BTreeSet::new()),
        Err(super::ACTIVATION_PARTIAL_REFUSED.to_owned())
    );
}

#[test]
fn missing_adapters_yield_truthful_health() {
    let effective = build_effective_defaults().expect("effective");
    let report = derive_readiness(
        &effective,
        shell_dependencies(),
        AcceptedReceipts::default(),
    );
    assert!(!report.composition.runtime_owner_ready);
    assert!(!report.composition.control_store_ready);
    assert!(!report.stores.direct_store_ready);
    assert!(!report.capabilities.source_backed_search_available);
    assert!(!report.capabilities.search_available);
    assert!(!report.stores.secret_store_ready);
}

#[test]
fn profile_self_authorization_is_rejected() {
    let registry = daemon_registry().expect("registry");
    let source = ConfigSource {
        kind: ConfigSourceKind::File,
        source_ref: ConfigSourceRef::new("self-auth", 128).expect("ref"),
        source_digest: Blake3Digest32::from_bytes([7; 32]),
    };
    let document = ConfigDocument::from_entries(
        1,
        Some(
            search_contracts::ProfileId::new("semantic_optional")
                .expect("profile"),
        ),
        source,
        [],
        search_config::ConfigLimits::W1,
    )
    .expect("document");
    let merged = search_config::merge_layers(
        search_config::ConfigLayers {
            defaults: ConfigSource {
                kind: ConfigSourceKind::File,
                source_ref: ConfigSourceRef::new("defaults-probe", 128)
                    .expect("ref"),
                source_digest: Blake3Digest32::from_bytes([0; 32]),
            },
            requested_profile: search_contracts::ProfileId::new(
                "semantic_optional",
            )
            .expect("profile"),
            file: Some(document),
            environment: None,
            cli: None,
        },
        &registry,
        search_config::ConfigLimits::W1,
    );
    assert!(merged.is_err());
}

#[test]
fn read_only_status_writes_nothing_and_leaks_nothing() {
    let effective = build_effective(
        Some(direct_file(
            "[instance]\ndata_root = \"C:/Users/alice/private-search-data\"\n[secrets]\nqdrant_api_secret_ref = \"secret://credential/qdrant-production\"\n",
        )),
        None,
        None,
        "direct",
        "direct",
    )
    .expect("effective");
    let report = derive_readiness(
        &effective,
        direct_dependencies(),
        AcceptedReceipts::default(),
    );
    let first = config_status_json(&effective, &report);
    assert!(first.contains("\"read_only\":true"));
    assert!(first.contains("\"search_available\":false"));
    assert!(!first.contains("alice"));
    assert!(!first.contains("qdrant-production"));
    for _ in 0..1_000 {
        assert_eq!(config_status_json(&effective, &report), first);
    }
    assert!(first.contains(&sha256::hex(report.fingerprint.as_bytes())));
}

#[test]
fn identical_inputs_reproduce_identical_fingerprints() {
    let left = build_effective(
        Some(direct_file("[query]\nlimit = 21\n")),
        None,
        None,
        "direct",
        "direct",
    )
    .expect("left");
    let right = build_effective(
        Some(direct_file("[query]\nlimit = 21\n")),
        None,
        None,
        "direct",
        "direct",
    )
    .expect("right");
    assert_eq!(left.fingerprint(), right.fingerprint());
}

#[test]
fn secret_cli_injection_is_rejected_at_merge() {
    let cli = capture_cli_document(&[(
        "secrets.qdrant_api_secret_ref",
        "secret://credential/from-cli",
    )])
    .expect("cli builds");
    assert!(build_effective(None, None, cli, "direct", "direct").is_err());
}

#[test]
fn typed_cli_document_rejects_string_lists() {
    let path = ConfigKeyPath::new(
        search_config::ConfigSectionName::new("query", 128).expect("section"),
        search_config::ConfigKeyName::new("limit", 128).expect("key"),
    );
    let entries = vec![(
        path,
        LayerOperation::Set(DocumentValue::StringList(vec!["a".to_owned()])),
    )];
    assert!(super::capture_cli_typed_document(entries).is_err());
}

#[test]
fn cli_flags_parse_with_bounds_and_fail_closed() {
    let args = vec![
        "--serve-data-root".to_owned(),
        "/tmp/root".to_owned(),
        "--config-file".to_owned(),
        "/tmp/eliot.toml".to_owned(),
        "--set".to_owned(),
        "query.limit=40".to_owned(),
        "--set".to_owned(),
        "query.limit=41".to_owned(),
    ];
    let (remaining, cli) = super::parse_cli_config_args(&args).expect("parse");
    assert_eq!(remaining, vec!["--serve-data-root", "/tmp/root"]);
    assert_eq!(
        cli.config_file,
        Some(std::path::PathBuf::from("/tmp/eliot.toml"))
    );
    assert_eq!(
        cli.overrides,
        vec![
            ("query.limit".to_owned(), "40".to_owned()),
            ("query.limit".to_owned(), "41".to_owned()),
        ]
    );
    assert!(super::parse_cli_config_args(&["--config-file".to_owned()]).is_err());
    assert!(
        super::parse_cli_config_args(&[
            "--set".to_owned(),
            "querylimit40".to_owned(),
        ])
        .is_err()
    );
    assert!(
        super::parse_cli_config_args(&[
            "--set".to_owned(),
            "query.limit=".to_owned(),
        ])
        .is_err()
    );
    assert!(
        super::parse_cli_config_args(&[
            "--config-file".to_owned(),
            "a.toml".to_owned(),
            "--config-file".to_owned(),
            "b.toml".to_owned(),
        ])
        .is_err()
    );
}

#[test]
fn startup_layers_precede_defaults_file_env_cli() {
    let file = direct_file("[query]\nlimit = 20\n");
    let environment =
        capture_environment_document(&[("ELIOT_SEARCH__QUERY__LIMIT", "30")])
            .expect("env")
            .expect("layer");
    let cli = capture_cli_document(&[("query.limit", "40")])
        .expect("cli")
        .expect("layer");
    let effective = build_effective(
        Some(file),
        Some(environment),
        Some(cli),
        "direct",
        "direct",
    )
    .expect("layered");
    let section =
        search_config::ConfigSectionName::new("query", 128).expect("section");
    let key = search_config::ConfigKeyName::new("limit", 128).expect("key");
    let field = effective
        .snapshot()
        .section(&section)
        .expect("section")
        .field(&key)
        .expect("field");
    assert_eq!(field.value, search_config::ConfigValue::Integer(40));
}

#[test]
fn startup_partial_refuses_and_retains_defaults_without_leaks() {
    let current = build_effective_defaults().expect("current");
    let candidate = build_effective(
        Some(direct_file("[query]\nlimit = 11\n")),
        None,
        None,
        "direct",
        "direct",
    )
    .expect("candidate");
    assert_ne!(current.fingerprint(), candidate.fingerprint());
    assert_eq!(
        try_activate(&current, candidate, &BTreeSet::new()),
        Err(super::ACTIVATION_PARTIAL_REFUSED.to_owned())
    );
    let secret_cli = vec![
        "--set".to_owned(),
        "secrets.qdrant_api_secret_ref=hunter2".to_owned(),
    ];
    let (_, cli) = super::parse_cli_config_args(&secret_cli).expect("parse");
    assert_eq!(cli.overrides.len(), 1);
    let bad_overrides = [(
        "secrets.qdrant_api_secret_ref".to_owned(),
        "hunter2".to_owned(),
    )];
    let borrowed = bad_overrides
        .iter()
        .map(|(dotted, value)| (dotted.as_str(), value.as_str()))
        .collect::<Vec<_>>();
    let layer = capture_cli_document(&borrowed).expect("layer builds");
    let refused = build_effective(None, None, layer, "direct", "direct");
    assert!(refused.is_err());
    let message = format!("{}", refused.expect_err("must fail"));
    assert!(!message.contains("hunter2"));
    let file_error = super::read_config_file_bytes(std::path::Path::new(
        "/definitely/missing/eliot-test-config.toml",
    ));
    assert!(file_error.is_err());
    assert!(!file_error.expect_err("must fail").contains("/definitely/missing"));
}
