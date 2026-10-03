use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use xtask::control_record_instance_profiles::{
    ControlRecordInstanceProfileError, validate_control_record_instance_profiles,
};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

const INPUTS: &[&str] = &[
    "swarm/control-plane-schema.toml",
    "swarm/schemas/context-manifest-v1.toml",
    "swarm/schemas/assignment-ticket-v1.toml",
    "swarm/schemas/writer-lease-v1.toml",
    "swarm/schemas/lease-event-v1.toml",
    "swarm/context-manifest-instance-v1.toml",
    "swarm/assignment-ticket-instance-v1.toml",
    "swarm/writer-lease-instance-v1.toml",
    "swarm/lease-event-instance-v1.toml",
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
            "eliot-search-instance-profile-{}-{ordinal}",
            std::process::id()
        ));
        std::fs::create_dir(&root).expect("unique isolated fixture directory");

        let source_root = repository_root();
        for relative in INPUTS {
            let source = source_root.join(relative);
            let target = root.join(relative);
            std::fs::create_dir_all(target.parent().expect("fixture input parent"))
                .expect("fixture input directory");
            std::fs::copy(source, target).expect("copy exact control-profile input");
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
fn checked_in_first_four_profiles_validate_as_non_authoritative() {
    let report = validate_control_record_instance_profiles(&repository_root())
        .expect("the exact first-four profiles are structurally closed");

    assert_eq!(report.authority, "NON_AUTHORITATIVE");
    assert_eq!(report.scope, "FIRST_FOUR_INSTANCE_PROFILE_STRUCTURE_ONLY");
    assert_eq!(report.max_total_input_bytes, 288 * 1024);
    assert_eq!(
        report.record_kinds,
        vec![
            "context_manifest_v1",
            "assignment_ticket_v1",
            "writer_lease_v1",
            "lease_event_v1",
        ]
    );
}

#[test]
fn context_profile_is_loaded_from_its_registry_binding() {
    let fixture = IsolatedFixture::new();
    fixture.replace_once(
        "swarm/context-manifest-instance-v1.toml",
        "instance_status = \"MATERIALIZED\"",
        "instance_status = \"ISSUED\"",
    );

    assert_eq!(
        validate_control_record_instance_profiles(&fixture.root).unwrap_err(),
        ControlRecordInstanceProfileError::ProfileDefinitionInvalid
    );
}

#[test]
fn ticket_lease_and_event_statuses_are_exact_and_closed() {
    for (path, before, after) in [
        (
            "swarm/assignment-ticket-instance-v1.toml",
            "instance_status = \"ISSUED\"",
            "instance_status = \"LEASED\"",
        ),
        (
            "swarm/writer-lease-instance-v1.toml",
            "instance_status = \"LEASED\"",
            "instance_status = \"ISSUED\"",
        ),
        (
            "swarm/lease-event-instance-v1.toml",
            "instance_status = \"RECORDED\"",
            "instance_status = \"ACKNOWLEDGED\"",
        ),
        (
            "swarm/assignment-ticket-instance-v1.toml",
            "instance_status = \"ISSUED\"",
            "instance_status = \"FUTURE_STATUS\"",
        ),
    ] {
        let fixture = IsolatedFixture::new();
        fixture.replace_once(path, before, after);
        assert_eq!(
            validate_control_record_instance_profiles(&fixture.root).unwrap_err(),
            ControlRecordInstanceProfileError::ProfileDefinitionInvalid
        );
    }
}

#[test]
fn profile_shape_rejects_unknown_keys_and_unresolved_instance_statuses() {
    let unknown_key = IsolatedFixture::new();
    let profile = "swarm/assignment-ticket-instance-v1.toml";
    let path = unknown_key.root.join(profile);
    let mut contents = std::fs::read_to_string(&path).expect("profile fixture is readable");
    contents.push_str("\nunspecified_authority = true\n");
    std::fs::write(path, contents).expect("write isolated profile mutation");
    assert_eq!(
        validate_control_record_instance_profiles(&unknown_key.root).unwrap_err(),
        ControlRecordInstanceProfileError::ProfileDefinitionInvalid
    );

    let placeholder = IsolatedFixture::new();
    placeholder.replace_once(
        profile,
        "instance_status = \"ISSUED\"",
        "instance_status = \"SCHEMA_ONLY_NOT_AN_INSTANCE\"",
    );
    assert_eq!(
        validate_control_record_instance_profiles(&placeholder.root).unwrap_err(),
        ControlRecordInstanceProfileError::ProfileDefinitionInvalid
    );

    let wrong_kind = IsolatedFixture::new();
    wrong_kind.replace_once(
        profile,
        "record_kind = \"assignment_ticket_v1\"",
        "record_kind = \"writer_lease_v1\"",
    );
    assert_eq!(
        validate_control_record_instance_profiles(&wrong_kind.root).unwrap_err(),
        ControlRecordInstanceProfileError::ProfileDefinitionInvalid
    );

    let wrong_profile_id = IsolatedFixture::new();
    wrong_profile_id.replace_once(
        profile,
        "profile = \"assignment_ticket_instance_v1\"",
        "profile = \"writer_lease_instance_v1\"",
    );
    assert_eq!(
        validate_control_record_instance_profiles(&wrong_profile_id.root).unwrap_err(),
        ControlRecordInstanceProfileError::ProfileDefinitionInvalid
    );

    let missing_profile_field = IsolatedFixture::new();
    missing_profile_field.replace_once(profile, "unknown_instance_status = \"reject\"\n", "");
    assert_eq!(
        validate_control_record_instance_profiles(&missing_profile_field.root).unwrap_err(),
        ControlRecordInstanceProfileError::ProfileDefinitionInvalid
    );

    let malformed_toml = IsolatedFixture::new();
    malformed_toml.replace_once(profile, "schema_version = 1", "schema_version = [");
    assert_eq!(
        validate_control_record_instance_profiles(&malformed_toml.root).unwrap_err(),
        ControlRecordInstanceProfileError::ProfileDefinitionInvalid
    );
}

#[test]
fn registry_requires_exact_separate_profile_list_and_unique_bindings() {
    let wrong_profile_list = IsolatedFixture::new();
    let registry_path = wrong_profile_list
        .root
        .join("swarm/control-plane-schema.toml");
    let contents = std::fs::read_to_string(&registry_path).expect("registry fixture is readable");
    let marker = "required_instance_profile_files = [";
    let (prefix, rest) = contents
        .split_once(marker)
        .expect("profile file list is present");
    let (profile_list, suffix) = rest.split_once(']').expect("profile file list is closed");
    let profile_lines = profile_list
        .lines()
        .filter(|line| !line.contains("lease-event-instance-v1.toml"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(
        registry_path,
        format!("{prefix}{marker}{profile_lines}]{suffix}"),
    )
    .expect("remove one profile from the isolated fixture list");
    assert_eq!(
        validate_control_record_instance_profiles(&wrong_profile_list.root).unwrap_err(),
        ControlRecordInstanceProfileError::RegistryBindingInvalid
    );

    let duplicate_binding = IsolatedFixture::new();
    let registry = duplicate_binding
        .root
        .join("swarm/control-plane-schema.toml");
    let mut contents = std::fs::read_to_string(&registry).expect("registry fixture is readable");
    contents.push_str(
        "\n[[record]]\nkind = \"assignment_ticket_v1\"\npath = \"swarm/schemas/assignment-ticket-v1.toml\"\ncanonical_layout = \"swarm/tickets/<package>/<ticket_id>.toml\"\nproducer = \"integration-owner\"\nconsumer = \"writer_lease_issuer\"\ninstance_profile = \"swarm/assignment-ticket-instance-v1.toml\"\n",
    );
    std::fs::write(registry, contents).expect("write isolated registry mutation");
    assert_eq!(
        validate_control_record_instance_profiles(&duplicate_binding.root).unwrap_err(),
        ControlRecordInstanceProfileError::RegistryBindingInvalid
    );

    let wrong_binding = IsolatedFixture::new();
    wrong_binding.replace_once(
        "swarm/control-plane-schema.toml",
        "instance_profile = \"swarm/assignment-ticket-instance-v1.toml\"",
        "instance_profile = \"swarm/writer-lease-instance-v1.toml\"",
    );
    assert_eq!(
        validate_control_record_instance_profiles(&wrong_binding.root).unwrap_err(),
        ControlRecordInstanceProfileError::RegistryBindingInvalid
    );

    let unknown_binding_key = IsolatedFixture::new();
    unknown_binding_key.replace_once(
        "swarm/control-plane-schema.toml",
        "consumer = \"writer_lease_issuer\"",
        "consumer = \"writer_lease_issuer\"\nextra_binding = \"unreviewed\"",
    );
    assert_eq!(
        validate_control_record_instance_profiles(&unknown_binding_key.root).unwrap_err(),
        ControlRecordInstanceProfileError::RegistryBindingInvalid
    );

    let missing_binding = IsolatedFixture::new();
    missing_binding.replace_once(
        "swarm/control-plane-schema.toml",
        "instance_profile = \"swarm/assignment-ticket-instance-v1.toml\"",
        "",
    );
    assert_eq!(
        validate_control_record_instance_profiles(&missing_binding.root).unwrap_err(),
        ControlRecordInstanceProfileError::RegistryBindingInvalid
    );
}

#[test]
fn missing_profile_file_is_rejected_by_the_read_only_validator() {
    let fixture = IsolatedFixture::new();
    std::fs::remove_file(fixture.root.join("swarm/lease-event-instance-v1.toml"))
        .expect("remove only the isolated fixture profile");

    assert_eq!(
        validate_control_record_instance_profiles(&fixture.root).unwrap_err(),
        ControlRecordInstanceProfileError::ProfileInputInvalid
    );
}

#[test]
fn oversized_profile_is_rejected_before_toml_parsing() {
    let fixture = IsolatedFixture::new();
    let path = fixture
        .root
        .join("swarm/assignment-ticket-instance-v1.toml");
    let mut contents = std::fs::read_to_string(&path).expect("profile fixture is readable");
    contents.push_str(&" ".repeat(4 * 1024));
    std::fs::write(path, contents).expect("write oversized isolated profile");

    assert_eq!(
        validate_control_record_instance_profiles(&fixture.root).unwrap_err(),
        ControlRecordInstanceProfileError::ProfileInputInvalid
    );
}
