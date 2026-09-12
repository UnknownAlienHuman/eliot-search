//! Structural regression gate for validator entrypoints already migrated to Rust.
//!
//! This test is intentionally narrower than the unfinished repository-wide T41
//! migration: it freezes completed slices so a later change cannot silently
//! restore Python or Node as the required runtime.

use std::fs;
use std::path::Path;

struct MigratedEntrypoint {
    wrapper: &'static str,
    command: &'static str,
    retired_python: &'static str,
}

const MIGRATED: &[MigratedEntrypoint] = &[
    MigratedEntrypoint {
        wrapper: "tools/validate-accepted-evidence-digest.ps1",
        command: "accepted-evidence-digest",
        retired_python: "tools/accepted_evidence_digest_v1.py",
    },
    MigratedEntrypoint {
        wrapper: "tools/validate-p00-ticket-drafts.ps1",
        command: "p00-ticket-drafts",
        retired_python: "tools/validate-p00-ticket-drafts.py",
    },
    MigratedEntrypoint {
        wrapper: "tools/validate-p00-foundation-acceptance.ps1",
        command: "p00-foundation-acceptance",
        retired_python: "tools/validate-p00-foundation-acceptance.py",
    },
    MigratedEntrypoint {
        wrapper: "tools/validate-implementation-program.ps1",
        command: "implementation-program",
        retired_python: "tools/validate-implementation-program.py",
    },
    MigratedEntrypoint {
        wrapper: "tools/validate-ticket-issuance-plan.ps1",
        command: "ticket-issuance-plan",
        retired_python: "tools/validate-ticket-issuance-plan.py",
    },
    MigratedEntrypoint {
        wrapper: "tools/validate-w1-agent-drafts.ps1",
        command: "w1-agent-drafts",
        retired_python: "tools/validate-w1-agent-drafts.py",
    },
    MigratedEntrypoint {
        wrapper: "tools/validate-w2-agent-drafts.ps1",
        command: "w2-agent-drafts",
        retired_python: "tools/validate-w2-agent-drafts.py",
    },
    MigratedEntrypoint {
        wrapper: "tools/validate-w3-agent-drafts.ps1",
        command: "w3-agent-drafts",
        retired_python: "tools/validate-w3-agent-drafts.py",
    },
    MigratedEntrypoint {
        wrapper: "tools/validate-w4-agent-drafts.ps1",
        command: "w4-agent-drafts",
        retired_python: "tools/validate-w4-agent-drafts.py",
    },
    MigratedEntrypoint {
        wrapper: "tools/validate-w1-milestone-packets.ps1",
        command: "w1-milestone-packets",
        retired_python: "tools/validate-w1-milestone-packets.py",
    },
    MigratedEntrypoint {
        wrapper: "tools/validate-w2-milestone-packets.ps1",
        command: "w2-milestone-packets",
        retired_python: "tools/validate-w2-milestone-packets.py",
    },
    MigratedEntrypoint {
        wrapper: "tools/validate-w3-milestone-packets.ps1",
        command: "w3-milestone-packets",
        retired_python: "tools/validate-w3-milestone-packets.py",
    },
];

#[test]
fn migrated_required_entrypoints_remain_rust_only() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must be a workspace member");

    for entrypoint in MIGRATED {
        let wrapper_path = root.join(entrypoint.wrapper);
        let wrapper = fs::read_to_string(&wrapper_path)
            .unwrap_or_else(|error| panic!("{}: {error}", wrapper_path.display()));
        let lower = wrapper.to_ascii_lowercase();

        assert!(
            lower.contains("cargo")
                && lower.contains("--locked")
                && lower.contains("xtask")
                && lower.contains("validate")
                && lower.contains(entrypoint.command),
            "{} no longer invokes the locked Rust validator {}",
            entrypoint.wrapper,
            entrypoint.command
        );
        assert!(
            !lower.contains("python")
                && !lower.contains("py -")
                && !lower.contains("node")
                && !lower.contains("npx"),
            "{} restored a Python/Node runtime dependency",
            entrypoint.wrapper
        );
        assert!(
            !root.join(entrypoint.retired_python).exists(),
            "retired implementation returned: {}",
            entrypoint.retired_python
        );
    }
}

#[test]
fn workflows_do_not_reference_retired_validator_files() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must be a workspace member");
    let workflows = root.join(".github/workflows");

    for entry in fs::read_dir(&workflows)
        .unwrap_or_else(|error| panic!("{}: {error}", workflows.display()))
    {
        let entry = entry.expect("workflow directory entry must be readable");
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        for migrated in MIGRATED {
            assert!(
                !text.contains(migrated.retired_python),
                "{} references retired {}",
                path.display(),
                migrated.retired_python
            );
        }
    }
}
