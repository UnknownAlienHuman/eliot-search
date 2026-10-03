//! Immutable repository selection, registry closure and output fencing.

use std::path::{Path, PathBuf};
use std::process::Command;

use toml::Value;

use crate::git_tree::{GitTree, GitTreeError};
use crate::ticket_planner::{
    CURRENT_PACKAGE_RECORD_ROOTS, PLAN_ARTIFACT_ROOT, RECORD_KIND,
    ROOT_METADATA_NAMES, actor_identity_valid, advisory_output_path_valid,
};

use super::model::{
    Checks, PlannerView, RegistrySnapshot, TicketIssuanceBuildError,
    TicketIssuanceBuildOptions,
};
use super::util::{boolean, count_string, file_name, integer, text, unique_row};

pub(super) fn open_view(
    root: &Path,
    options: &TicketIssuanceBuildOptions,
    checks: &mut Checks,
) -> Result<PlannerView, TicketIssuanceBuildError> {
    let state = validate_selection(options, checks);
    let tree = if let Some(base) = options.base_commit.as_deref() {
        match GitTree::open(root, base) {
            Ok(tree) => {
                checks.pass(
                    "git-view",
                    format!(
                        "all repository inputs read from immutable commit {}",
                        tree.tagged_commit()
                    ),
                );
                tree
            }
            Err(error) if error.reason() == "BASE_COMMIT_INVALID" => {
                checks.fail("base-commit", error.reason(), error.message());
                let tree = open_head(root)?;
                checks.pass(
                    "git-view-fallback",
                    format!(
                        "invalid selection inspected against immutable HEAD {}",
                        tree.tagged_commit()
                    ),
                );
                tree
            }
            Err(error) => return Err(map_git(error)),
        }
    } else {
        let tree = open_head(root)?;
        checks.pass(
            "git-view",
            format!(
                "all repository inputs read from immutable commit {}",
                tree.tagged_commit()
            ),
        );
        tree
    };
    Ok(PlannerView {
        tree,
        selection_state: state,
    })
}

fn validate_selection(
    options: &TicketIssuanceBuildOptions,
    checks: &mut Checks,
) -> &'static str {
    let count = [
        options.base_commit.as_ref(),
        options.writer.as_ref(),
        options.reviewer.as_ref(),
    ]
    .iter()
    .filter(|value| value.is_some())
    .count();
    if count == 0 {
        checks.pass("selection", "no issuance identity selected");
        return "NONE";
    }
    if count != 3 {
        checks.fail(
            "selection",
            "PARTIAL_ISSUANCE_SELECTION",
            "base commit, writer and reviewer must be supplied together",
        );
        return "PARTIAL";
    }
    let writer = options.writer.as_deref().unwrap_or_default();
    let reviewer = options.reviewer.as_deref().unwrap_or_default();
    if actor_identity_valid(writer) && actor_identity_valid(reviewer) {
        checks.pass(
            "actor-identities",
            "writer and reviewer use closed ActorIdentity grammar",
        );
    } else {
        checks.fail(
            "actor-identities",
            "ACTOR_IDENTITY_INVALID",
            "writer or reviewer ActorIdentity is invalid",
        );
    }
    if writer == reviewer {
        checks.fail(
            "actor-independence",
            "WRITER_REVIEWER_CONFLICT",
            "writer and reviewer are identical",
        );
    } else {
        checks.pass(
            "actor-independence",
            "writer and reviewer identities differ",
        );
    }
    "COMPLETE"
}

fn open_head(root: &Path) -> Result<GitTree, TicketIssuanceBuildError> {
    let canonical = std::fs::canonicalize(root).map_err(|error| {
        TicketIssuanceBuildError::new(
            "GIT_REPOSITORY_INVALID",
            format!("unable to canonicalize repository root: {error}"),
        )
    })?;
    let object_format = git_text(&canonical, &["rev-parse", "--show-object-format"])?;
    let oid = git_text(&canonical, &["rev-parse", "HEAD"])?;
    let tagged = format!("{}:{}", object_format.trim(), oid.trim().to_ascii_lowercase());
    GitTree::open(&canonical, &tagged).map_err(map_git)
}

fn git_text(root: &Path, args: &[&str]) -> Result<String, TicketIssuanceBuildError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|error| {
            TicketIssuanceBuildError::new(
                "GIT_REPOSITORY_INVALID",
                format!("unable to execute Git: {error}"),
            )
        })?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(TicketIssuanceBuildError::new(
            "GIT_REPOSITORY_INVALID",
            if detail.trim().is_empty() {
                "Git command failed".to_owned()
            } else {
                detail.trim().to_owned()
            },
        ));
    }
    String::from_utf8(output.stdout).map_err(|_| {
        TicketIssuanceBuildError::new(
            "GIT_REPOSITORY_INVALID",
            "Git command emitted non-UTF-8 output",
        )
    })
}

pub(super) fn validate_registries(
    tree: &GitTree,
    package: &str,
    checks: &mut Checks,
) -> RegistrySnapshot {
    let loaded = (|| {
        Ok::<_, GitTreeError>((
            tree.load_toml("swarm/launch-state.toml")?.0,
            tree.load_toml("swarm/crates.toml")?.0,
            tree.load_toml("swarm/function-packets.toml")?.0,
            tree.load_toml("swarm/stages.toml")?.0,
        ))
    })();
    let (launch, crates, functions, stages) = match loaded {
        Ok(values) => values,
        Err(error) => {
            checks.fail("registry-files", "CONTROL_SCHEMA_MISMATCH", error.message());
            return RegistrySnapshot {
                launch: empty_table(),
                package_row: None,
                function_row: None,
                stage_row: None,
            };
        }
    };

    let package_row = unique_row(&crates, "package", "name", package);
    let function_row = unique_row(&functions, "foundation", "package", package);
    let stage_row = unique_row(&stages, "stage", "id", "W0");

    if package_row.is_some() {
        checks.pass("package-registry", "unique package registry entry");
    } else {
        checks.fail(
            "package-registry",
            "PACKAGE_UNKNOWN",
            "package entry is missing or duplicate",
        );
    }
    if function_row.is_some() {
        checks.pass(
            "function-registry",
            "unique P00 foundation function entry",
        );
    } else {
        checks.fail(
            "function-registry",
            "PACKAGE_REGISTRY_MISMATCH",
            "P00 foundation function entry is missing or duplicate",
        );
    }
    let stage_ok = stage_row.as_ref().is_some_and(|row| {
        count_string(row.get("packages"), package) == 1
    });
    if stage_ok {
        checks.pass("stage-registry", "package belongs exactly once to W0");
    } else {
        checks.fail(
            "stage-registry",
            "PACKAGE_STAGE_MISMATCH",
            "package is not exactly once in W0",
        );
    }

    if let (Some(package_row), Some(function_row)) =
        (package_row.as_ref(), function_row.as_ref())
    {
        let path = text(package_row, "path");
        let scope = path.map(|value| format!("{value}/**")).unwrap_or_default();
        let coherent = path.is_some_and(crate::ticket_planner::safe_path)
            && text(package_row, "family") == Some("foundation")
            && integer(package_row, "wave") == Some(0)
            && integer(function_row, "wave") == Some(0)
            && function_row.get("assignment") == package_row.get("assignment")
            && text(function_row, "write_scope") == Some(scope.as_str());
        if coherent {
            checks.pass("package-scope", format!("package-only write scope: {scope}"));
        } else {
            checks.fail(
                "package-scope",
                "PACKAGE_REGISTRY_MISMATCH",
                "path/family/wave/assignment/write-scope mismatch",
            );
        }
    }

    if integer(&launch, "schema_version") == Some(6)
        && text(&launch, "active_stage") == Some("P00")
        && integer(&launch, "active_wave") == Some(0)
    {
        checks.pass("launch-stage", "launch state remains P00/W0");
    } else {
        checks.fail(
            "launch-stage",
            "PACKAGE_STAGE_MISMATCH",
            "launch state is not schema-v6 P00/W0",
        );
    }

    RegistrySnapshot {
        launch,
        package_row,
        function_row,
        stage_row,
    }
}

pub(super) fn validate_control_schema(
    tree: &GitTree,
    launch: &Value,
    checks: &mut Checks,
) {
    let required = [
        ("swarm/orchestration.toml", 5_i64),
        ("swarm/control-plane-schema.toml", 3),
        ("swarm/schemas/types-v1.toml", 2),
        ("swarm/ticket-issuance-plan-schema-v2.toml", 2),
        ("swarm/ticket-issuance-plan-digest-v2.toml", 2),
        ("swarm/ticket-issuance-planner-v2.toml", 2),
        ("swarm/p00-foundation-acceptance.toml", 1),
    ];
    let mut documents = std::collections::BTreeMap::new();
    for (path, version) in required {
        match tree.load_toml(path) {
            Ok((document, _)) => {
                if integer(&document, "schema_version") != Some(version) {
                    checks.fail(
                        "control-schema",
                        "CONTROL_SCHEMA_MISMATCH",
                        "planner/control/orchestration schema or path mismatch",
                    );
                    return;
                }
                documents.insert(path, document);
            }
            Err(error) => {
                checks.fail("control-schema", "CONTROL_SCHEMA_MISMATCH", error.message());
                return;
            }
        }
    }
    let schema = &documents["swarm/ticket-issuance-plan-schema-v2.toml"];
    let digest = &documents["swarm/ticket-issuance-plan-digest-v2.toml"];
    let registry = &documents["swarm/ticket-issuance-planner-v2.toml"];
    let orchestration = &documents["swarm/orchestration.toml"];
    let coherent = text(schema, "record_kind") == Some(RECORD_KIND)
        && boolean(digest, "self_referential_digest_allowed") == Some(false)
        && text(registry, "component") == Some("ticket_issuance_planner_v2")
        && text(orchestration, "workflow_policy") == Some("manual_only")
        && boolean(orchestration, "consumer_uses_branch_head") == Some(false)
        && boolean(
            orchestration,
            "consumer_requires_exact_commit_and_api_digest",
        ) == Some(true)
        && integer(launch, "orchestration_registry_schema_version") == Some(5)
        && text(launch, "orchestration_registry_path")
            == Some("swarm/orchestration.toml");
    if coherent {
        checks.pass(
            "control-schema",
            "planner, control, orchestration and acceptance schemas agree",
        );
    } else {
        checks.fail(
            "control-schema",
            "CONTROL_SCHEMA_MISMATCH",
            "planner/control/orchestration schema or path mismatch",
        );
    }
}

pub(super) fn validate_control_state(
    tree: &GitTree,
    package: &str,
    checks: &mut Checks,
) -> Result<(), TicketIssuanceBuildError> {
    for root in crate::ticket_planner::CONTROL_ROOTS {
        let files = tree.list_files(root).map_err(map_git)?;
        let root_metadata: Vec<String> = ROOT_METADATA_NAMES
            .iter()
            .map(|name| format!("{root}/{name}"))
            .collect();
        let found: Vec<&str> = files
            .iter()
            .map(String::as_str)
            .filter(|path| root_metadata.iter().any(|expected| expected == path))
            .collect();
        if found.is_empty() {
            checks.fail(
                format!("root-metadata:{root}"),
                "CONTROL_SCHEMA_MISMATCH",
                format!("control root lacks exact root metadata: {root}"),
            );
        } else {
            checks.pass(
                format!("root-metadata:{root}"),
                format!("root metadata present: {}", found.join(",")),
            );
        }
        let nested = files.iter().find(|path| {
            !root_metadata.iter().any(|expected| expected == *path)
                && ROOT_METADATA_NAMES.contains(&file_name(path))
        });
        if let Some(path) = nested {
            checks.fail(
                format!("root-nested-metadata:{root}"),
                "CURRENT_PACKAGE_CONTROL_RECORD_EXISTS",
                format!("nested metadata filename is a record, not an exemption: {path}"),
            );
        } else {
            checks.pass(
                format!("root-nested-metadata:{root}"),
                "no nested metadata filename bypass",
            );
        }
    }

    let mut conflicts = Vec::new();
    for root in CURRENT_PACKAGE_RECORD_ROOTS {
        conflicts.extend(
            tree.list_files(&format!("{root}/{package}"))
                .map_err(map_git)?,
        );
    }
    conflicts.sort();
    if let Some(path) = conflicts.first() {
        checks.fail(
            "current-package-records",
            "CURRENT_PACKAGE_CONTROL_RECORD_EXISTS",
            format!("current-package control record already exists: {path}"),
        );
    } else {
        checks.pass(
            "current-package-records",
            "no current-package context/ticket/lease/submission/review/handoff record",
        );
    }

    let wave_records: Vec<String> = tree
        .list_files("swarm/wave-receipts")
        .map_err(map_git)?
        .into_iter()
        .filter(|path| {
            !matches!(
                path.as_str(),
                "swarm/wave-receipts/README.md" | "swarm/wave-receipts/.gitkeep"
            )
        })
        .collect();
    if let Some(path) = wave_records.first() {
        checks.fail(
            "w0-receipt",
            "W0_ALREADY_ACCEPTED",
            format!("wave receipt already exists: {path}"),
        );
    } else {
        checks.pass("w0-receipt", "no accepted wave receipt exists");
    }
    Ok(())
}

pub(super) fn validate_workflows(
    tree: &GitTree,
    checks: &mut Checks,
) -> Result<(), TicketIssuanceBuildError> {
    let files: Vec<String> = tree
        .list_files(".github/workflows")
        .map_err(map_git)?
        .into_iter()
        .filter(|path| path.ends_with(".yml") || path.ends_with(".yaml"))
        .collect();
    let mut violations = Vec::new();
    for path in &files {
        match tree.read_text(path) {
            Ok((text, _)) if workflow_is_manual_read_only(&text) => {}
            _ => violations.push(path.clone()),
        }
    }
    if !files.is_empty() && violations.is_empty() {
        checks.pass(
            "workflow-policy",
            format!(
                "{} workflows are manual/read-only/credential-free",
                files.len()
            ),
        );
    } else {
        checks.fail(
            "workflow-policy",
            "WORKFLOW_POLICY_VIOLATION",
            format!(
                "workflow policy violation: {}",
                violations.first().map_or("none found", String::as_str)
            ),
        );
    }
    Ok(())
}

fn workflow_is_manual_read_only(text: &str) -> bool {
    const FORBIDDEN: [&str; 20] = [
        "push:",
        "pull_request:",
        "pull_request_target:",
        "merge_group:",
        "schedule:",
        "workflow_run:",
        "repository_dispatch:",
        "workflow_call:",
        "release:",
        "issues:",
        "issue_comment:",
        "discussion:",
        "discussion_comment:",
        "create:",
        "delete:",
        "check_run:",
        "check_suite:",
        "deployment:",
        "deployment_status:",
        "status:",
    ];
    let mut manual = false;
    let mut contents_read = false;
    let mut contents_write = false;
    let mut forbidden = false;
    for line in text.lines() {
        let indent = line.len().saturating_sub(line.trim_start().len());
        let trimmed = line.trim();
        if indent == 2 && trimmed == "workflow_dispatch:" {
            manual = true;
        }
        if indent == 2 && trimmed == "contents: read" {
            contents_read = true;
        }
        if indent == 2 && trimmed == "contents: write" {
            contents_write = true;
        }
        if indent <= 6 && FORBIDDEN.contains(&trimmed) {
            forbidden = true;
        }
    }
    manual
        && !forbidden
        && contents_read
        && !contents_write
        && text.contains("persist-credentials: false")
}

pub(super) fn validate_output(
    root: &Path,
    output: &str,
    checks: &mut Checks,
) -> Option<PathBuf> {
    if output == "-" {
        checks.pass("output-path", "stdout selected");
        return None;
    }
    let relative = output.replace('\\', "/");
    if !advisory_output_path_valid(&relative) {
        checks.fail(
            "output-path",
            "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT",
            format!("output must be JSON below {PLAN_ARTIFACT_ROOT}"),
        );
        return None;
    }
    let target = root.join(&relative);
    let mut cursor = target.parent();
    while let Some(path) = cursor {
        if path == root {
            break;
        }
        if path.symlink_metadata().is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            checks.fail(
                "output-path",
                "OUTPUT_PATH_SYMLINK",
                format!(
                    "output parent is a symlink: {}",
                    path.file_name().and_then(|name| name.to_str()).unwrap_or("?")
                ),
            );
            return None;
        }
        cursor = path.parent();
    }
    if target
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        checks.fail(
            "output-path",
            "OUTPUT_PATH_SYMLINK",
            "output target is a symlink",
        );
        return None;
    }
    checks.pass(
        "output-path",
        format!("ordinary advisory artifact path: {relative}"),
    );
    Some(target)
}

fn empty_table() -> Value {
    Value::Table(toml::map::Map::new())
}

pub(super) fn map_git(error: GitTreeError) -> TicketIssuanceBuildError {
    TicketIssuanceBuildError::new(error.reason(), error.message())
}
