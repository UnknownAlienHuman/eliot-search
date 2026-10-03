use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use xtask::qualified_profile_id_registry::{
    MAX_TOTAL_INPUT_BYTES, QualifiedProfileIdRegistryError,
    validate_adopted_qualified_profile_id_field_value, validate_qualified_profile_id_registry,
};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

const TYPE_REGISTRY: &str = "swarm/schemas/types-v1.toml";
const CONTROL_REGISTRY: &str = "swarm/control-plane-schema.toml";
const INPUTS: &[&str] = &[CONTROL_REGISTRY, TYPE_REGISTRY];

const BINDINGS: &[(&str, &str)] = &[
    ("ImmutableArtifactRef", "store_profile_ref"),
    ("ImmutableSignatureRef", "approval_profile_ref"),
];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is nested under the repository root")
        .to_owned()
}

struct IsolatedFixture {
    root: PathBuf,
}

impl IsolatedFixture {
    fn new() -> Self {
        let ordinal = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "eliot-search-qualified-profile-id-{}-{ordinal}",
            std::process::id()
        ));
        std::fs::create_dir(&root).expect("unique isolated fixture directory");

        let source_root = repository_root();
        for relative in INPUTS {
            let source = source_root.join(relative);
            let target = root.join(relative);
            std::fs::create_dir_all(target.parent().expect("fixture input parent"))
                .expect("fixture input directory");
            std::fs::copy(source, target).expect("copy exact registry input");
        }

        Self { root }
    }

    fn replace_once(&self, relative: &str, before: &str, after: &str) {
        let path = self.root.join(relative);
        let contents = std::fs::read_to_string(&path).expect("fixture source is readable");
        assert_eq!(contents.matches(before).count(), 1, "unique fixture anchor");
        std::fs::write(path, contents.replacen(before, after, 1))
            .expect("write isolated fixture mutation");
    }

    fn replace_field_in_type(
        &self,
        relative: &str,
        type_name: &str,
        before: &str,
        after: &str,
    ) {
        let path = self.root.join(relative);
        let contents = std::fs::read_to_string(&path).expect("fixture source is readable");
        let marker = format!("name = \"{type_name}\"");
        assert_eq!(contents.matches(&marker).count(), 1, "unique type marker");
        let name_start = contents.find(&marker).expect("type marker is present");
        let block_start = contents[..name_start]
            .rfind("[[type]]")
            .expect("type block begins before its name");
        let next_block = contents[name_start..]
            .find("[[type]]")
            .map(|offset| name_start + offset)
            .unwrap_or(contents.len());
        let block = &contents[block_start..next_block];
        assert_eq!(block.matches(before).count(), 1, "unique type field");
        let replaced_block = block.replacen(before, after, 1);

        let mut updated = contents;
        updated.replace_range(block_start..next_block, &replaced_block);
        std::fs::write(path, updated).expect("write isolated type-field mutation");
    }
    fn duplicate_line_once(&self, relative: &str, line: &str) {
        let path = self.root.join(relative);
        let contents = std::fs::read_to_string(&path).expect("fixture source is readable");
        assert_eq!(contents.matches(line).count(), 1, "unique fixture rule");
        let newline = if contents.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let replacement = format!("{line}{newline}{line}");
        std::fs::write(path, contents.replacen(line, &replacement, 1))
            .expect("write duplicate isolated binding");
    }
}

impl Drop for IsolatedFixture {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir().canonicalize();
        let fixture_root = self.root.canonicalize();
        if let (Ok(temp_root), Ok(fixture_root)) = (temp_root, fixture_root)
            && fixture_root.starts_with(&temp_root)
            && fixture_root != temp_root
        {
            let _ = std::fs::remove_dir_all(fixture_root);
        }
    }
}

#[test]
fn checked_in_registry_is_exact_and_non_authoritative() {
    let report = validate_qualified_profile_id_registry(&repository_root())
        .expect("accepted qualified identifier type and bindings are exact");

    assert_eq!(report.authority, "NON_AUTHORITATIVE");
    assert_eq!(report.scope, "QUALIFIED_OPAQUE_ID_SYNTAX_BINDINGS_ONLY");
    assert_eq!(report.max_total_input_bytes, 80 * 1024);
    assert_eq!(report.declared_type_count, 52);
    assert_eq!(report.binding_count, 2);
}

#[test]
fn pure_value_checker_accepts_both_bindings_and_component_boundaries() {
    let max_namespace = format!("{}/X", "a".repeat(96));
    let max_local = format!("a/X{}", "_".repeat(127));
    let max_total = format!("{}/X{}", "a".repeat(96), "_".repeat(127));
    assert_eq!(max_total.len(), 225);

    for &(owner, field) in BINDINGS {
        for value in [
            "a/A",
            "acme-core/Spec.V1_ABC-9",
            max_namespace.as_str(),
            max_local.as_str(),
            max_total.as_str(),
        ] {
            assert_eq!(
                validate_adopted_qualified_profile_id_field_value(owner, field, value),
                Ok(()),
                "accepted qualified token for {owner}.{field}: {value}"
            );
        }
    }
}

#[test]
fn pure_value_checker_rejects_unknown_bindings_and_invalid_alias_tokens() {
    for (owner, field) in [
        ("UnreviewedOwner", "store_profile_ref"),
        ("ImmutableArtifactRef", "approval_profile_ref"),
        ("ImmutableSignatureRef", "store_profile_ref"),
        ("ImmutableArtifactRef", "store_profile"),
    ] {
        assert_eq!(
            validate_adopted_qualified_profile_id_field_value(owner, field, "acme/id"),
            Err(QualifiedProfileIdRegistryError::FieldBindingInvalid),
            "unknown binding {owner}.{field}"
        );
    }

    let namespace_97 = format!("{}/x", "a".repeat(97));
    let local_129 = format!("a/A{}", "_".repeat(128));
    let total_226 = format!("{}/A{}", "a".repeat(96), "_".repeat(128));
    for value in [
        "Alpha/x",
        "alpha/",
        "alpha/x/y",
        "alpha%2Fcore/id",
        "alpha/id%2Fextra",
        "é/id",
        "alpha/é",
        "alpha /id",
        namespace_97.as_str(),
        local_129.as_str(),
        total_226.as_str(),
    ] {
        assert_eq!(
            validate_adopted_qualified_profile_id_field_value(
                "ImmutableArtifactRef",
                "store_profile_ref",
                value,
            ),
            Err(QualifiedProfileIdRegistryError::QualifiedValueInvalid),
            "invalid exact qualified token: {value:?}"
        );
    }
}

#[test]
fn qualified_type_must_be_unique_exact_non_alias_and_component_bounded() {
    let missing = IsolatedFixture::new();
    missing.replace_once(TYPE_REGISTRY, "name = \"QualifiedOpaqueId\"", "name = \"UnknownQualifiedId\"");
    assert_eq!(
        validate_qualified_profile_id_registry(&missing.root).unwrap_err(),
        QualifiedProfileIdRegistryError::QualifiedTypeDefinitionInvalid
    );

    let duplicate = IsolatedFixture::new();
    duplicate.replace_once(TYPE_REGISTRY, "name = \"OpaqueId\"", "name = \"QualifiedOpaqueId\"");
    assert_eq!(
        validate_qualified_profile_id_registry(&duplicate.root).unwrap_err(),
        QualifiedProfileIdRegistryError::QualifiedTypeDefinitionInvalid
    );

    let wrong_pattern = IsolatedFixture::new();
    wrong_pattern.replace_once(
        TYPE_REGISTRY,
        "pattern = \"^[a-z][a-z0-9]*(?:-[a-z0-9]+)*/[A-Za-z0-9][A-Za-z0-9._-]{0,127}$\"",
        "pattern = \"^[A-Za-z0-9]+/[A-Za-z0-9]+$\"",
    );
    assert_eq!(
        validate_qualified_profile_id_registry(&wrong_pattern.root).unwrap_err(),
        QualifiedProfileIdRegistryError::QualifiedTypeDefinitionInvalid
    );

    let wrong_namespace_limit = IsolatedFixture::new();
    wrong_namespace_limit.replace_once(TYPE_REGISTRY, "namespace_max_bytes = 96", "namespace_max_bytes = 95");
    assert_eq!(
        validate_qualified_profile_id_registry(&wrong_namespace_limit.root).unwrap_err(),
        QualifiedProfileIdRegistryError::QualifiedTypeDefinitionInvalid
    );

    let wrong_local_limit = IsolatedFixture::new();
    wrong_local_limit.replace_once(TYPE_REGISTRY, "local_max_bytes = 128", "local_max_bytes = 127");
    assert_eq!(
        validate_qualified_profile_id_registry(&wrong_local_limit.root).unwrap_err(),
        QualifiedProfileIdRegistryError::QualifiedTypeDefinitionInvalid
    );

    let wrong_total_limit = IsolatedFixture::new();
    wrong_total_limit.replace_once(TYPE_REGISTRY, "max_bytes = 225", "max_bytes = 224");
    assert_eq!(
        validate_qualified_profile_id_registry(&wrong_total_limit.root).unwrap_err(),
        QualifiedProfileIdRegistryError::QualifiedTypeDefinitionInvalid
    );

    let alias_representation = IsolatedFixture::new();
    alias_representation.replace_field_in_type(
        TYPE_REGISTRY,
        "QualifiedOpaqueId",
        "representation = \"string\"",
        "representation = \"OpaqueId\"",
    );
    assert_eq!(
        validate_qualified_profile_id_registry(&alias_representation.root).unwrap_err(),
        QualifiedProfileIdRegistryError::QualifiedTypeDefinitionInvalid
    );

    let alias_key = IsolatedFixture::new();
    alias_key.replace_once(
        TYPE_REGISTRY,
        "name = \"QualifiedOpaqueId\"",
        "name = \"QualifiedOpaqueId\"\nalias_of = \"OpaqueId\"",
    );
    assert_eq!(
        validate_qualified_profile_id_registry(&alias_key.root).unwrap_err(),
        QualifiedProfileIdRegistryError::QualifiedTypeDefinitionInvalid
    );
}

#[test]
fn field_rules_are_owner_specific_unique_and_keep_their_field_order() {
    let missing = IsolatedFixture::new();
    missing.replace_once(
        TYPE_REGISTRY,
        "store_profile_ref_is_qualified_opaque_id",
        "store_profile_ref_is_OpaqueId",
    );
    assert_eq!(
        validate_qualified_profile_id_registry(&missing.root).unwrap_err(),
        QualifiedProfileIdRegistryError::FieldBindingInvalid
    );

    let duplicate = IsolatedFixture::new();
    duplicate.duplicate_line_once(
        TYPE_REGISTRY,
        "  \"store_profile_ref_is_qualified_opaque_id\",",
    );
    assert_eq!(
        validate_qualified_profile_id_registry(&duplicate.root).unwrap_err(),
        QualifiedProfileIdRegistryError::FieldBindingInvalid
    );

    let cross_bound = IsolatedFixture::new();
    cross_bound.replace_once(
        TYPE_REGISTRY,
        "store_profile_ref_is_qualified_opaque_id",
        "approval_profile_ref_is_qualified_opaque_id",
    );
    assert_eq!(
        validate_qualified_profile_id_registry(&cross_bound.root).unwrap_err(),
        QualifiedProfileIdRegistryError::FieldBindingInvalid
    );

    let reordered_fields = IsolatedFixture::new();
    reordered_fields.replace_once(
        TYPE_REGISTRY,
        "canonical_fields = [\"store_profile_ref\", \"artifact_id\", \"bytes\", \"sha256\"]",
        "canonical_fields = [\"artifact_id\", \"store_profile_ref\", \"bytes\", \"sha256\"]",
    );
    assert_eq!(
        validate_qualified_profile_id_registry(&reordered_fields.root).unwrap_err(),
        QualifiedProfileIdRegistryError::FieldBindingInvalid
    );
}

#[test]
fn control_registry_count_must_be_in_current_disposition() {
    let wrong_count = IsolatedFixture::new();
    wrong_count.replace_once(CONTROL_REGISTRY, "registered_types = 52", "registered_types = 51");
    assert_eq!(
        validate_qualified_profile_id_registry(&wrong_count.root).unwrap_err(),
        QualifiedProfileIdRegistryError::RegistryDefinitionInvalid
    );

    let root_only_count = IsolatedFixture::new();
    root_only_count.replace_once(
        CONTROL_REGISTRY,
        "registered_types = 52",
        "disposition_registered_types = 52",
    );
    root_only_count.replace_once(
        CONTROL_REGISTRY,
        "schema_version = 4",
        "registered_types = 52\nschema_version = 4",
    );
    assert_eq!(
        validate_qualified_profile_id_registry(&root_only_count.root).unwrap_err(),
        QualifiedProfileIdRegistryError::RegistryDefinitionInvalid
    );
}

#[test]
fn arrays_and_alias_keys_in_the_qualified_definition_fail_closed() {
    let malformed_rules = IsolatedFixture::new();
    malformed_rules.replace_once(
        TYPE_REGISTRY,
        "\"exactly_one_ascii_forward_slash\", \"namespace_uses_lowercase_ascii_hyphen_segments\"",
        "\"exactly_one_ascii_forward_slash\" \"namespace_uses_lowercase_ascii_hyphen_segments\"",
    );
    assert_eq!(
        validate_qualified_profile_id_registry(&malformed_rules.root).unwrap_err(),
        QualifiedProfileIdRegistryError::RegistryDefinitionInvalid
    );

    let unknown_rule = IsolatedFixture::new();
    unknown_rule.replace_once(
        TYPE_REGISTRY,
        "\"preserve_exact_bytes_and_case_without_normalization\"",
        "\"preserve_exact_bytes_and_case_without_normalization\", \"looks_like_a_rule_alias\"",
    );
    assert_eq!(
        validate_qualified_profile_id_registry(&unknown_rule.root).unwrap_err(),
        QualifiedProfileIdRegistryError::QualifiedTypeDefinitionInvalid
    );
}