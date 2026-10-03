use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use xtask::control_record_enum_bindings::{
    ControlRecordEnumBindingsError, validate_control_record_enum_bindings,
};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

const INPUTS: &[&str] = &[
    "swarm/control-plane-schema.toml",
    "swarm/schemas/types-v1.toml",
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
            "eliot-search-enum-bindings-{}-{ordinal}",
            std::process::id()
        ));
        std::fs::create_dir(&root).expect("unique isolated fixture directory");

        let source_root = repository_root();
        for relative in INPUTS {
            let source = source_root.join(relative);
            let target = root.join(relative);
            std::fs::create_dir_all(target.parent().expect("fixture input parent"))
                .expect("fixture input directory");
            std::fs::copy(source, target).expect("copy exact enum-binding input");
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

    fn insert_line_after_once(&self, relative: &str, line: &str, inserted: &str) {
        let path = self.root.join(relative);
        let contents = std::fs::read_to_string(&path).expect("fixture source is readable");
        assert_eq!(contents.matches(line).count(), 1, "unique fixture anchor");
        let newline = if contents.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let replacement = format!("{line}{newline}{inserted}");
        std::fs::write(path, contents.replacen(line, &replacement, 1))
            .expect("write isolated additional binding");
    }

    fn append_to_types(&self, suffix: &str) {
        let path = self.root.join("swarm/schemas/types-v1.toml");
        let mut contents = std::fs::read_to_string(&path).expect("type fixture is readable");
        contents.push_str(suffix);
        std::fs::write(path, contents).expect("write isolated type-registry mutation");
    }

    fn remove_input(&self, relative: &str) {
        std::fs::remove_file(self.root.join(relative)).expect("remove isolated fixture input");
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
fn checked_in_enum_definitions_and_nine_bindings_validate_non_authoritatively() {
    let report = validate_control_record_enum_bindings(&repository_root())
        .expect("accepted enum definitions and bindings are structurally closed");

    assert_eq!(report.authority, "NON_AUTHORITATIVE");
    assert_eq!(report.scope, "CONTROL_RECORD_ENUM_BINDINGS_ONLY");
    assert_eq!(report.max_total_input_bytes, 80 * 1024);
    assert_eq!(report.declared_type_count, 51);
    assert_eq!(report.enum_count, 4);
    assert_eq!(report.binding_count, 9);
}

#[test]
fn enum_allowlists_are_ordered_exact_and_reject_unknown_fields() {
    let wrong_values = IsolatedFixture::new();
    wrong_values.replace_once(
        "swarm/schemas/types-v1.toml",
        "allowed = [\"FAILED\", \"QUALIFIED\", \"UNAVAILABLE\"]",
        "allowed = [\"FAILED\", \"QUALIFIED\", \"UNAVAILABLE\", \"PENDING\"]",
    );
    assert_eq!(
        validate_control_record_enum_bindings(&wrong_values.root).unwrap_err(),
        ControlRecordEnumBindingsError::EnumDefinitionInvalid
    );

    let wrong_order = IsolatedFixture::new();
    wrong_order.replace_once(
        "swarm/schemas/types-v1.toml",
        "allowed = [\"EXIT_NONZERO\", \"EXIT_ZERO\"]",
        "allowed = [\"EXIT_ZERO\", \"EXIT_NONZERO\"]",
    );
    assert_eq!(
        validate_control_record_enum_bindings(&wrong_order.root).unwrap_err(),
        ControlRecordEnumBindingsError::EnumDefinitionInvalid
    );

    let missing_enum = IsolatedFixture::new();
    missing_enum.replace_once(
        "swarm/schemas/types-v1.toml",
        "name = \"FixtureQualificationStatus\"",
        "name = \"UnregisteredFixtureStatus\"",
    );
    assert_eq!(
        validate_control_record_enum_bindings(&missing_enum.root).unwrap_err(),
        ControlRecordEnumBindingsError::EnumDefinitionInvalid
    );
    let unknown_key = IsolatedFixture::new();
    unknown_key.replace_once(
        "swarm/schemas/types-v1.toml",
        "name = \"FixtureQualificationStatus\"",
        "name = \"FixtureQualificationStatus\"\nunreviewed = true",
    );
    assert_eq!(
        validate_control_record_enum_bindings(&unknown_key.root).unwrap_err(),
        ControlRecordEnumBindingsError::EnumDefinitionInvalid
    );
}

#[test]
fn missing_wrong_duplicate_and_cross_bound_rules_are_rejected() {
    let missing_binding = IsolatedFixture::new();
    missing_binding.replace_once(
        "swarm/schemas/types-v1.toml",
        "qualification_status_is_FixtureQualificationStatus",
        "qualification_status_is_ClosedEnum",
    );
    assert_eq!(
        validate_control_record_enum_bindings(&missing_binding.root).unwrap_err(),
        ControlRecordEnumBindingsError::BindingInvalid
    );

    let wrong_type = IsolatedFixture::new();
    wrong_type.replace_once(
        "swarm/schemas/types-v1.toml",
        "qualification_status_is_FixtureQualificationStatus",
        "qualification_status_is_NormalProcessExitClass",
    );
    assert_eq!(
        validate_control_record_enum_bindings(&wrong_type.root).unwrap_err(),
        ControlRecordEnumBindingsError::BindingInvalid
    );

    let duplicate_binding = IsolatedFixture::new();
    duplicate_binding.duplicate_line_once(
        "swarm/schemas/types-v1.toml",
        "  \"qualification_status_is_FixtureQualificationStatus\",",
    );
    assert_eq!(
        validate_control_record_enum_bindings(&duplicate_binding.root).unwrap_err(),
        ControlRecordEnumBindingsError::BindingInvalid
    );

    let cross_bound_exit_field = IsolatedFixture::new();
    cross_bound_exit_field.replace_once(
        "swarm/schemas/types-v1.toml",
        "expected_exit_class_is_NormalProcessExitClass",
        "evidence_class_is_NormalProcessExitClass",
    );
    assert_eq!(
        validate_control_record_enum_bindings(&cross_bound_exit_field.root).unwrap_err(),
        ControlRecordEnumBindingsError::BindingInvalid
    );

    let unknown_binding = IsolatedFixture::new();
    unknown_binding.insert_line_after_once(
        "swarm/schemas/types-v1.toml",
        "  \"qualification_status_is_FixtureQualificationStatus\",",
        "  \"unreviewed_is_FixtureQualificationStatus\",",
    );
    assert_eq!(
        validate_control_record_enum_bindings(&unknown_binding.root).unwrap_err(),
        ControlRecordEnumBindingsError::BindingInvalid
    );
}

#[test]
fn duplicate_enum_definition_is_rejected() {
    let fixture = IsolatedFixture::new();
    fixture.replace_once(
        "swarm/schemas/types-v1.toml",
        "name = \"OpaqueId\"",
        "name = \"FixtureQualificationStatus\"",
    );

    assert_eq!(
        validate_control_record_enum_bindings(&fixture.root).unwrap_err(),
        ControlRecordEnumBindingsError::EnumDefinitionInvalid,
        "duplicate adopted type name must fail closed"
    );
}

#[test]
fn registry_pin_and_type_registry_size_are_bounded() {
    let wrong_registry_pin = IsolatedFixture::new();
    wrong_registry_pin.replace_once(
        "swarm/control-plane-schema.toml",
        "registered_types = 51",
        "registered_types = 50",
    );
    assert_eq!(
        validate_control_record_enum_bindings(&wrong_registry_pin.root).unwrap_err(),
        ControlRecordEnumBindingsError::ControlRegistryDefinitionInvalid
    );

    let wrong_status = IsolatedFixture::new();
    wrong_status.replace_once(
        "swarm/schemas/types-v1.toml",
        "status = \"SCHEMA_ONLY_NOT_IMPLEMENTED\"",
        "status = \"IMPLEMENTED\"",
    );
    assert_eq!(
        validate_control_record_enum_bindings(&wrong_status.root).unwrap_err(),
        ControlRecordEnumBindingsError::TypeRegistryDefinitionInvalid
    );

    let missing_registry = IsolatedFixture::new();
    missing_registry.remove_input("swarm/schemas/types-v1.toml");
    assert_eq!(
        validate_control_record_enum_bindings(&missing_registry.root).unwrap_err(),
        ControlRecordEnumBindingsError::TypeRegistryInputInvalid
    );

    let oversized = IsolatedFixture::new();
    oversized.append_to_types(&" ".repeat(64 * 1024));
    assert_eq!(
        validate_control_record_enum_bindings(&oversized.root).unwrap_err(),
        ControlRecordEnumBindingsError::TypeRegistryInputInvalid
    );
}
