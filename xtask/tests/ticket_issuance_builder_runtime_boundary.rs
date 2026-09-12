use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is nested under repository root")
        .to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn required_ticket_planner_entrypoint_is_locked_rust() {
    let root = repository_root();
    let wrapper = read(&root, "tools/plan-ticket-issuance.ps1");
    let lower = wrapper.to_ascii_lowercase();
    for token in [
        "cargo",
        "--locked",
        "xtask",
        "build",
        "ticket-issuance-plan",
    ] {
        assert!(lower.contains(token), "wrapper missing {token}");
    }
    for forbidden in [
        "python",
        "plan-ticket-issuance.py",
        "py -",
        "node",
        "npx",
    ] {
        assert!(
            !lower.contains(forbidden),
            "wrapper restored forbidden runtime token {forbidden}"
        );
    }

    let workflow = read(&root, ".github/workflows/ticket-issuance-plan.yml");
    assert!(workflow.contains("workflow_dispatch:"));
    assert!(workflow.contains("contents: read"));
    assert!(workflow.contains("persist-credentials: false"));
    assert!(workflow.contains("ticket_issuance_builder"));
    assert!(!workflow.to_ascii_lowercase().contains("python"));

    let registry = read(&root, "swarm/ticket-issuance-planner-v2.toml");
    assert!(registry.contains(
        "implementation = \"xtask/src/ticket_issuance_builder.rs\""
    ));
    assert!(registry.contains(
        "xtask/src/ticket_issuance_builder/assemble.rs"
    ));
    assert!(registry.contains(
        "xtask/src/ticket_issuance_builder/assemble/plan.rs"
    ));
    assert!(!registry.contains(
        "implementation = \"tools/plan-ticket-issuance.py\""
    ));
}

#[test]
fn ticket_builder_stays_bounded_and_non_authoritative() {
    let root = repository_root();
    let facade = read(&root, "xtask/src/ticket_issuance_builder.rs");
    assert!(
        facade.len() < 2_000,
        "builder facade grew to {} bytes",
        facade.len()
    );
    for module in [
        "assemble",
        "context",
        "control",
        "drafts",
        "model",
        "repository",
        "util",
        "write",
    ] {
        assert!(facade.contains(&format!("mod {module};")));
    }
    assert!(!facade.contains("std::fs"));
    assert!(!facade.contains("Command::new"));

    let assemble = read(
        &root,
        "xtask/src/ticket_issuance_builder/assemble.rs",
    );
    assert!(assemble.contains("mod plan;"));
    assert!(assemble.contains("let decision = choose_decision("));
    assert!(!assemble.contains("\"mutations\": []"));

    let plan = read(
        &root,
        "xtask/src/ticket_issuance_builder/assemble/plan.rs",
    );
    assert!(plan.contains("\"mutations\": []"));
    for field in [
        "authorizes_context_materialization",
        "authorizes_ticket_issuance",
        "creates_writer_lease",
        "authorizes_implementation",
        "publishes_package_handoff",
        "advances_launch_state",
    ] {
        assert!(plan.contains(&format!("\"{field}\": false")));
    }
    assert!(!plan.contains("swarm/tickets/"));
    assert!(!plan.contains("swarm/leases/"));
}
