//! Immutable-tree preflight for one context-artifact candidate.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::{Value as JsonValue, json};
use toml::Value;

use crate::context_artifact::{ARTIFACT_FORMAT, ARTIFACT_ROOT, RECORD_KIND};
use crate::git_tree::{GitTree, GitTreeError};
use crate::ticket_planner::{
    exact_sha256_hex, opaque_id_valid, package_name_valid, safe_path,
    sha256_hex_valid, under,
};

use super::model::{
    AcceptedHandoff, CandidateCheck, ContextArtifactBuildError, DraftPair,
    Preflight,
};

const CONTROL_ROOTS: [&str; 8] = [
    "swarm/context-manifests",
    "swarm/tickets",
    "swarm/leases",
    "swarm/submissions",
    "swarm/reviews",
    "swarm/handoffs",
    "swarm/supersessions",
    "swarm/wave-receipts",
];

/// Runs a read-only preflight against one exact committed Git tree.
pub(super) fn run(
    root: &Path,
    package: &str,
    base_commit: &str,
    accepted_handoff_paths: &[String],
) -> Result<Preflight, ContextArtifactBuildError> {
    if !package_name_valid(package) {
        return Err(ContextArtifactBuildError::new(
            "PACKAGE_UNKNOWN",
            "package does not use the closed package-name grammar",
        ));
    }
    let tree = GitTree::open(root, base_commit).map_err(map_git_without_checks)?;
    let mut checks = Vec::new();

    validate_builder_contract(&tree, &mut checks)?;
    let package_row = unique_row(
        &load_toml(&tree, "swarm/crates.toml", &checks)?,
        "package",
        "name",
        package,
        "PACKAGE_UNKNOWN",
        &checks,
    )?;
    let package_path = text(&package_row, "path").ok_or_else(|| {
        failure(
            &checks,
            "PACKAGE_REGISTRY_MISMATCH",
            "package registry path is missing",
        )
    })?;
    let function_row = unique_row(
        &load_toml(&tree, "swarm/function-packets.toml", &checks)?,
        "foundation",
        "package",
        package,
        "PACKAGE_REGISTRY_MISMATCH",
        &checks,
    )?;
    let stage_row = unique_row(
        &load_toml(&tree, "swarm/stages.toml", &checks)?,
        "stage",
        "id",
        "W0",
        "PACKAGE_STAGE_MISMATCH",
        &checks,
    )?;
    let launch = load_toml(&tree, "swarm/launch-state.toml", &checks)?;
    let function_scope = text(&function_row, "write_scope");
    let expected_scope = format!("{package_path}/**");
    require(
        function_scope == Some(expected_scope.as_str())
            && integer(&function_row, "wave") == Some(0)
            && text(&stage_row, "phase") == Some("P00"),
        &mut checks,
        "registry-parity",
        "PACKAGE_REGISTRY_MISMATCH",
        "package/function/stage registry rows are coherent",
    )?;

    let classification = launch_class(&launch, package).ok_or_else(|| {
        failure(
            &checks,
            "PACKAGE_STAGE_MISMATCH",
            "package is neither authorized nor conditional in launch state",
        )
    })?;
    pass(
        &mut checks,
        "launch-class",
        &format!("package launch class is {classification}"),
    );

    let pair = load_draft_pair(
        &tree,
        package,
        package_path,
        classification,
        &checks,
    )?;
    pass(
        &mut checks,
        "draft-pair",
        "ticket/context draft pair is exact, non-claimable and unresolved",
    );

    validate_control_roots(&tree, package, &mut checks)?;
    validate_workflows(&tree, &mut checks)?;
    let handoffs = validate_handoffs(
        &tree,
        &pair,
        accepted_handoff_paths,
        &mut checks,
    )?;

    Ok(Preflight {
        tree,
        pair,
        package_path: package_path.to_owned(),
        handoffs,
        checks,
    })
}

fn validate_builder_contract(
    tree: &GitTree,
    checks: &mut Vec<CandidateCheck>,
) -> Result<(), ContextArtifactBuildError> {
    let registry = load_toml(tree, "swarm/context-artifact-builder-v1.toml", checks)?;
    let schema = load_toml(
        tree,
        "swarm/context-artifact-candidate-schema-v1.toml",
        checks,
    )?;
    let digest = load_toml(
        tree,
        "swarm/context-artifact-candidate-digest-v1.toml",
        checks,
    )?;
    let authority = registry.get("authority").and_then(Value::as_table);
    let coherent = integer(&registry, "schema_version") == Some(1)
        && text(&registry, "component") == Some("context_artifact_builder_v1")
        && text(&registry, "candidate_schema")
            == Some("swarm/context-artifact-candidate-schema-v1.toml")
        && text(&registry, "digest_profile")
            == Some("swarm/context-artifact-candidate-digest-v1.toml")
        && text(&registry, "artifact_root") == Some(ARTIFACT_ROOT)
        && text(&registry, "artifact_format") == Some(ARTIFACT_FORMAT)
        && integer(&schema, "schema_version") == Some(1)
        && text(&schema, "record_kind") == Some(RECORD_KIND)
        && text(&schema, "artifact_format") == Some(ARTIFACT_FORMAT)
        && integer(&digest, "schema_version") == Some(1)
        && text(&digest, "profile")
            == Some("context_artifact_candidate_digest_v1")
        && boolean(&digest, "self_referential_digest_allowed") == Some(false)
        && authority.is_some_and(|table| {
            !table.is_empty()
                && table.values().all(|value| value.as_bool() == Some(false))
        });
    require(
        coherent,
        checks,
        "builder-contract",
        "CONTEXT_ARTIFACT_BUILDER_CONTRACT_MISMATCH",
        "builder registry, candidate schema and digest profile are coherent",
    )
}

fn load_draft_pair(
    tree: &GitTree,
    package: &str,
    package_path: &str,
    classification: &str,
    checks: &[CandidateCheck],
) -> Result<DraftPair, ContextArtifactBuildError> {
    let ticket_manifest = load_toml(tree, "swarm/ticket-drafts/manifest.toml", checks)?;
    let context_manifest = load_toml(tree, "swarm/context-drafts/manifest.toml", checks)?;
    if integer(&ticket_manifest, "schema_version") != Some(2)
        || integer(&context_manifest, "schema_version") != Some(2)
        || count_rows(&ticket_manifest, "draft") != integer(&ticket_manifest, "draft_count")
        || count_rows(&context_manifest, "draft") != integer(&context_manifest, "draft_count")
    {
        return Err(failure(
            checks,
            "DRAFT_MANIFEST_MISMATCH",
            "draft manifest schema or count mismatch",
        ));
    }
    let ticket_row = unique_row(
        &ticket_manifest,
        "draft",
        "package",
        package,
        "DRAFT_PAIR_MISSING",
        checks,
    )?;
    let context_row = unique_row(
        &context_manifest,
        "draft",
        "package",
        package,
        "DRAFT_PAIR_MISSING",
        checks,
    )?;
    let ticket_path = text(&ticket_row, "path").ok_or_else(|| {
        failure(checks, "DRAFT_PAIR_MISSING", "ticket draft path is missing")
    })?;
    let context_path = text(&context_row, "path").ok_or_else(|| {
        failure(checks, "DRAFT_PAIR_MISSING", "context draft path is missing")
    })?;
    let source_ceiling_class = text(&context_row, "source_ceiling_class")
        .ok_or_else(|| {
            failure(
                checks,
                "DRAFT_MANIFEST_MISMATCH",
                "source ceiling class is missing",
            )
        })?;
    if !safe_path(ticket_path) || !safe_path(context_path) {
        return Err(failure(
            checks,
            "DRAFT_PAIR_MISMATCH",
            "draft path is not repository-relative safe",
        ));
    }
    let (ticket_raw, _) = tree.read_bytes(ticket_path).map_err(|error| map_git(error, checks))?;
    let (context_raw, context_entry) = tree
        .read_bytes(context_path)
        .map_err(|error| map_git(error, checks))?;
    let ticket = parse_toml_bytes(&ticket_raw, "DRAFT_PAIR_MISMATCH", checks)?;
    let context = parse_toml_bytes(&context_raw, "DRAFT_PAIR_MISMATCH", checks)?;

    let ticket_context = ticket.get("context").and_then(Value::as_table);
    let identity_ok = integer(&ticket, "schema_version") == Some(2)
        && integer(&context, "schema_version") == Some(2)
        && text(&ticket, "package") == Some(package)
        && text(&context, "package") == Some(package)
        && text(&ticket, "stage") == Some("W0")
        && text(&context, "stage") == Some("W0")
        && text(&ticket, "phase") == Some("P00")
        && text(&context, "phase") == Some("P00")
        && integer(&ticket, "wave") == Some(0)
        && integer(&context, "wave") == Some(0)
        && ticket_context
            .and_then(|table| table.get("context_draft"))
            .and_then(Value::as_str)
            == Some(context_path);
    if !identity_ok {
        return Err(failure(
            checks,
            "DRAFT_PAIR_MISMATCH",
            "ticket/context package, schema or stage mismatch",
        ));
    }
    let nonclaimable = text(&ticket, "record_kind") == Some("assignment_ticket_draft")
        && text(&ticket, "status") == Some("DRAFT_ONLY_NOT_ISSUED")
        && boolean(&ticket, "claimable") == Some(false)
        && boolean(&ticket, "authorizes_implementation") == Some(false)
        && boolean(&ticket, "creates_lease") == Some(false)
        && boolean(&ticket, "may_be_writer_acknowledged") == Some(false)
        && text(&context, "record_kind") == Some("writer_context_draft")
        && text(&context, "status") == Some("UNMATERIALIZED_DRAFT")
        && boolean(&context, "claimable") == Some(false)
        && boolean(&context, "authorizes_implementation") == Some(false);
    if !nonclaimable {
        return Err(failure(
            checks,
            "DRAFT_BECAME_CLAIMABLE",
            "draft authority flags are unsafe",
        ));
    }
    let unresolved = ticket.get("unresolved_identity").and_then(Value::as_table);
    let unresolved_ok = unresolved.is_some_and(|table| {
        table.get("ticket_id").and_then(Value::as_str) == Some("UNASSIGNED")
            && table.get("writer").and_then(Value::as_str) == Some("UNASSIGNED")
            && table.get("reviewer").and_then(Value::as_str) == Some("UNASSIGNED")
            && table.get("issued_at").and_then(Value::as_str) == Some("")
            && table.get("base_commit").and_then(Value::as_str) == Some("UNSELECTED")
            && table.get("branch_or_worktree").and_then(Value::as_str)
                == Some("UNSELECTED")
    }) && text(&context, "base_commit") == Some("UNSELECTED")
        && text(&context, "materialized_context_manifest_ref") == Some("UNAVAILABLE")
        && text(&context, "materialized_context_artifact_ref") == Some("UNAVAILABLE");
    if !unresolved_ok {
        return Err(failure(
            checks,
            "DRAFT_IDENTITY_PREMATURELY_RESOLVED",
            "draft contains premature issuance identity",
        ));
    }
    let repository_fence = ticket.get("repository_fence").and_then(Value::as_table);
    let expected_scope = format!("{package_path}/**");
    let fence_ok = repository_fence.is_some_and(|table| {
        table.get("repository").and_then(Value::as_str)
            == Some("UnknownAlienHuman/eliot-search")
            && table.get("write_scope").and_then(Value::as_str)
                == Some(expected_scope.as_str())
            && table.get("feature_profile").and_then(Value::as_str)
                == Some("P00_FOUNDATION")
    }) && text(&ticket, "launch_class") == Some(classification);
    if !fence_ok {
        return Err(failure(
            checks,
            "DRAFT_PAIR_MISMATCH",
            "repository fence or launch class mismatch",
        ));
    }

    let content = context.get("content").and_then(Value::as_table).ok_or_else(|| {
        failure(checks, "DRAFT_PAIR_MISMATCH", "missing [content] table")
    })?;
    let sources = string_array(content.get("source_files"), "source_files", checks)?;
    let selectors = string_array(
        content.get("registry_fragments"),
        "registry_fragments",
        checks,
    )?;
    let handoff_slots = string_array(
        content.get("accepted_handoff_slots"),
        "accepted_handoff_slots",
        checks,
    )?;
    let unavailable_checks = string_array(
        content.get("required_unavailable_checks"),
        "required_unavailable_checks",
        checks,
    )?;
    let forbidden = string_array(
        content.get("forbidden_paths"),
        "forbidden_paths",
        checks,
    )?;
    let ceiling = if source_ceiling_class == "P00_EXACT_CONTRACT_PACK" {
        integer(&context_manifest, "p00_exact_contract_pack_source_file_ceiling")
    } else {
        integer(&context_manifest, "ordinary_static_source_file_ceiling")
    };
    let fragment_ceiling = integer(&context_manifest, "max_registry_fragments_per_context");
    let handoff_ceiling = integer(&context_manifest, "max_accepted_handoff_slots_per_context");
    let counts_ok = ceiling.is_some_and(|value| usize::try_from(value).is_ok_and(|value| sources.len() <= value))
        && fragment_ceiling.is_some_and(|value| usize::try_from(value).is_ok_and(|value| selectors.len() <= value))
        && handoff_ceiling.is_some_and(|value| usize::try_from(value).is_ok_and(|value| handoff_slots.len() <= value))
        && integer(&context, "source_file_count") == i64::try_from(sources.len()).ok()
        && integer(&context, "registry_fragment_count") == i64::try_from(selectors.len()).ok()
        && integer(&context, "accepted_handoff_slot_count") == i64::try_from(handoff_slots.len()).ok()
        && unique(&sources)
        && unique(&selectors)
        && unique(&handoff_slots)
        && unique(&unavailable_checks);
    if !counts_ok {
        return Err(failure(
            checks,
            "CONTEXT_BUDGET_EXCEEDED",
            "context counts, uniqueness or manifest-owned ceilings mismatch",
        ));
    }
    let expected_slots: Vec<String> = if package == "search-contracts" {
        Vec::new()
    } else {
        vec!["search-contracts::accepted_package_and_api_handoff".to_owned()]
    };
    if handoff_slots != expected_slots {
        return Err(failure(
            checks,
            "DRAFT_PAIR_MISMATCH",
            "accepted-handoff slots differ from P00 dependency topology",
        ));
    }
    let canonical = context.get("canonicalization").and_then(Value::as_table);
    let canonical_ok = canonical.is_some_and(|table| {
        table.get("encoding").and_then(Value::as_str) == Some("UTF-8")
            && table.get("line_endings").and_then(Value::as_str) == Some("LF")
            && table.get("path_header_format").and_then(Value::as_str)
                == Some("--- repository-path: <path> ---")
            && table.get("registry_header_format").and_then(Value::as_str)
                == Some("--- registry-selector: <path>::<selector> ---")
            && table.get("preserve_declared_order").and_then(Value::as_bool) == Some(true)
            && table.get("record_source_sha256").and_then(Value::as_bool) == Some(true)
            && table.get("record_fragment_sha256").and_then(Value::as_bool) == Some(true)
    }) && text(&context, "materialization_mode") == Some("canonical_concatenated_bundle")
        && !unavailable_checks.is_empty()
        && forbidden.iter().any(|item| item == "docs/architecture/**")
        && sources.iter().all(|path| safe_path(path))
        && selectors.iter().all(|selector| selector_path_safe(selector));
    if !canonical_ok {
        return Err(failure(
            checks,
            "DRAFT_PAIR_MISMATCH",
            "context canonicalization, paths or unavailable checks mismatch",
        ));
    }

    Ok(DraftPair {
        ticket_path: ticket_path.to_owned(),
        context_path: context_path.to_owned(),
        ticket,
        context,
        sources,
        selectors,
        handoff_slots,
        unavailable_checks,
        source_ceiling_class: source_ceiling_class.to_owned(),
        context_blob: tree.blob_identity(&context_entry),
        context_sha256: exact_sha256_hex(&context_raw),
    })
}

fn validate_control_roots(
    tree: &GitTree,
    package: &str,
    checks: &mut Vec<CandidateCheck>,
) -> Result<(), ContextArtifactBuildError> {
    for root in CONTROL_ROOTS {
        let files = tree.list_files(root).map_err(|error| map_git(error, checks))?;
        let has_metadata = files.iter().any(|path| {
            path == &format!("{root}/README.md") || path == &format!("{root}/.gitkeep")
        });
        require(
            has_metadata,
            checks,
            &format!("root-metadata:{root}"),
            "CONTROL_SCHEMA_MISMATCH",
            "control root carries exact root metadata",
        )?;
        let nested_metadata = files.iter().any(|path| {
            path != &format!("{root}/README.md")
                && path != &format!("{root}/.gitkeep")
                && (path.ends_with("/README.md") || path.ends_with("/.gitkeep"))
        });
        require(
            !nested_metadata,
            checks,
            &format!("root-nested-metadata:{root}"),
            "CURRENT_PACKAGE_CONTROL_RECORD_EXISTS",
            "no nested metadata filename bypass",
        )?;
    }
    for root in &CONTROL_ROOTS[..6] {
        let prefix = format!("{root}/{package}");
        let files = tree.list_files(&prefix).map_err(|error| map_git(error, checks))?;
        if !files.is_empty() {
            return Err(failure(
                checks,
                "CURRENT_PACKAGE_CONTROL_RECORD_EXISTS",
                &format!("current-package control record already exists: {}", files[0]),
            ));
        }
    }
    pass(
        checks,
        "current-package-records",
        "no current-package control record exists",
    );
    let wave_records: Vec<String> = tree
        .list_files("swarm/wave-receipts")
        .map_err(|error| map_git(error, checks))?
        .into_iter()
        .filter(|path| {
            path != "swarm/wave-receipts/README.md"
                && path != "swarm/wave-receipts/.gitkeep"
        })
        .collect();
    require(
        wave_records.is_empty(),
        checks,
        "w0-receipt",
        "W0_ALREADY_ACCEPTED",
        "no accepted W0 receipt exists",
    )
}

fn validate_workflows(
    tree: &GitTree,
    checks: &mut Vec<CandidateCheck>,
) -> Result<(), ContextArtifactBuildError> {
    let workflows = tree
        .list_files(".github/workflows")
        .map_err(|error| map_git(error, checks))?
        .into_iter()
        .filter(|path| path.ends_with(".yml") || path.ends_with(".yaml"))
        .collect::<Vec<_>>();
    if workflows.is_empty() {
        return Err(failure(
            checks,
            "WORKFLOW_POLICY_VIOLATION",
            "no workflow files found",
        ));
    }
    for path in &workflows {
        let (text, _) = tree.read_text(path).map_err(|error| map_git(error, checks))?;
        let valid = text.lines().any(|line| line == "  workflow_dispatch:")
            && text.lines().any(|line| line == "  contents: read")
            && text.contains("persist-credentials: false")
            && !text.lines().any(|line| {
                matches!(
                    line,
                    "  push:"
                        | "  pull_request:"
                        | "  schedule:"
                        | "  workflow_run:"
                        | "  repository_dispatch:"
                )
            })
            && !text.lines().any(|line| line == "  contents: write");
        if !valid {
            return Err(failure(
                checks,
                "WORKFLOW_POLICY_VIOLATION",
                &format!("workflow policy violation: {path}"),
            ));
        }
    }
    pass(
        checks,
        "workflow-policy",
        &format!("{} workflows are manual/read-only/credential-free", workflows.len()),
    );
    Ok(())
}

fn validate_handoffs(
    tree: &GitTree,
    pair: &DraftPair,
    paths: &[String],
    checks: &mut Vec<CandidateCheck>,
) -> Result<Vec<AcceptedHandoff>, ContextArtifactBuildError> {
    let expected: Vec<String> = pair
        .handoff_slots
        .iter()
        .filter_map(|slot| slot.split_once("::").map(|(package, _)| package.to_owned()))
        .collect();
    if paths.len() != expected.len() {
        return Err(failure(
            checks,
            if paths.len() < expected.len() {
                "HANDOFF_SLOT_UNSATISFIED"
            } else {
                "HANDOFF_SET_UNEXPECTED"
            },
            "accepted handoff set differs from draft slots",
        ));
    }
    let superseded = superseded_handoffs(tree, checks)?;
    let mut supplied = BTreeMap::new();
    for (index, path) in paths.iter().enumerate() {
        if !safe_path(path) || !under(path, "swarm/handoffs") {
            return Err(failure(
                checks,
                "HANDOFF_RECORD_INVALID",
                &format!("unsafe handoff path: {path}"),
            ));
        }
        let (raw, entry) = tree.read_bytes(path).map_err(|error| map_git(error, checks))?;
        let record = parse_toml_bytes(&raw, "HANDOFF_RECORD_INVALID", checks)?;
        let identity = record.get("identity").and_then(Value::as_table).ok_or_else(|| {
            failure(checks, "HANDOFF_RECORD_INVALID", "handoff identity is missing")
        })?;
        let accepted = record.get("accepted_code").and_then(Value::as_table).ok_or_else(|| {
            failure(checks, "HANDOFF_RECORD_INVALID", "handoff accepted_code is missing")
        })?;
        let public = record.get("public_surface").and_then(Value::as_table).ok_or_else(|| {
            failure(checks, "HANDOFF_RECORD_INVALID", "handoff public_surface is missing")
        })?;
        let signature = record.get("signature").and_then(Value::as_table).ok_or_else(|| {
            failure(checks, "HANDOFF_RECORD_INVALID", "handoff signature is missing")
        })?;
        let package = identity.get("package").and_then(Value::as_str).unwrap_or_default();
        let handoff_id = identity.get("handoff_id").and_then(Value::as_str).unwrap_or_default();
        let final_commit = accepted.get("final_commit").and_then(Value::as_str).unwrap_or_default();
        let api = public.get("api_schema_digest").and_then(Value::as_str).unwrap_or_default();
        let reasons = public.get("error_reason_digest").and_then(Value::as_str).unwrap_or_default();
        let record_digest = signature.get("record_sha256").and_then(Value::as_str).unwrap_or_default();
        let valid = package_name_valid(package)
            && opaque_id_valid(handoff_id)
            && path == format!("swarm/handoffs/{package}/{handoff_id}.toml")
            && integer(&record, "schema_version") == Some(1)
            && text(&record, "record_kind") == Some("package_handoff_v1")
            && text(&record, "status") == Some("ACCEPTED")
            && identity.get("stage").and_then(Value::as_str) == Some("W0")
            && tree.commit_exists(final_commit)
            && sha256_hex_valid(api)
            && sha256_hex_valid(reasons)
            && signed_payload_digest(&raw).as_deref() == Some(record_digest)
            && !superseded.contains(path);
        if !valid || supplied.contains_key(package) {
            return Err(failure(
                checks,
                if superseded.contains(path) {
                    "HANDOFF_RECORD_SUPERSEDED"
                } else {
                    "HANDOFF_RECORD_INVALID"
                },
                &format!("handoff failed canonical checks: {path}"),
            ));
        }
        let summary = json!({
            "package": package,
            "path": path,
            "handoff_id": handoff_id,
            "git_blob_id": tree.blob_identity(&entry),
            "exact_record_file_sha256": exact_sha256_hex(&raw),
            "accepted_commit": final_commit,
            "api_schema_digest": api,
            "error_reason_digest": reasons,
        });
        supplied.insert(package.to_owned(), AcceptedHandoff { summary, bytes: raw });
        pass(
            checks,
            &format!("handoff-input-{index:02}"),
            &format!("canonical accepted handoff: {path}"),
        );
    }
    if supplied.keys().cloned().collect::<Vec<_>>() != expected {
        return Err(failure(
            checks,
            "HANDOFF_SET_UNEXPECTED",
            "accepted handoff package set differs from draft slots",
        ));
    }
    pass(
        checks,
        "handoff-set",
        "accepted handoff package set exactly matches draft slots",
    );
    Ok(supplied.into_values().collect())
}

fn superseded_handoffs(
    tree: &GitTree,
    checks: &[CandidateCheck],
) -> Result<BTreeSet<String>, ContextArtifactBuildError> {
    let mut result = BTreeSet::new();
    for path in tree
        .list_files("swarm/supersessions")
        .map_err(|error| map_git(error, checks))?
    {
        if path.ends_with("/README.md") || path.ends_with("/.gitkeep") {
            continue;
        }
        let Ok((record, _)) = tree.load_toml(&path) else {
            continue;
        };
        let reference = record
            .get("old_record")
            .and_then(Value::as_table)
            .and_then(|table| table.get("ref"))
            .and_then(Value::as_table)
            .and_then(|table| table.get("path"))
            .and_then(Value::as_str);
        if let Some(reference) = reference {
            result.insert(reference.to_owned());
        }
    }
    Ok(result)
}

fn signed_payload_digest(raw: &[u8]) -> Option<String> {
    const MARKER: &[u8] = b"\n[signature]\n";
    let positions = raw
        .windows(MARKER.len())
        .enumerate()
        .filter_map(|(index, window)| (window == MARKER).then_some(index))
        .collect::<Vec<_>>();
    if positions.len() != 1 || positions[0] == 0 {
        return None;
    }
    Some(exact_sha256_hex(&raw[..positions[0] + 1]))
}

fn launch_class<'a>(launch: &'a Value, package: &str) -> Option<&'a str> {
    if contains_once(launch.get("authorized_packages"), package) {
        Some("AUTHORIZED")
    } else if contains_once(launch.get("conditional_packages"), package) {
        Some("CONDITIONAL")
    } else {
        None
    }
}

fn contains_once(value: Option<&Value>, expected: &str) -> bool {
    value.and_then(Value::as_array).is_some_and(|items| {
        items.iter().filter(|item| item.as_str() == Some(expected)).count() == 1
    })
}

fn unique_row(
    document: &Value,
    array: &str,
    key: &str,
    expected: &str,
    reason: &str,
    checks: &[CandidateCheck],
) -> Result<Value, ContextArtifactBuildError> {
    let matches = document
        .get(array)
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter(|row| row.get(key).and_then(Value::as_str) == Some(expected))
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if matches.len() == 1 {
        Ok(matches.into_iter().next().expect("one row"))
    } else {
        Err(failure(
            checks,
            reason,
            &format!("{array}[{key}={expected}] did not resolve exactly once"),
        ))
    }
}

fn load_toml(
    tree: &GitTree,
    path: &str,
    checks: &[CandidateCheck],
) -> Result<Value, ContextArtifactBuildError> {
    tree.load_toml(path)
        .map(|(value, _)| value)
        .map_err(|error| map_git(error, checks))
}

fn parse_toml_bytes(
    raw: &[u8],
    reason: &str,
    checks: &[CandidateCheck],
) -> Result<Value, ContextArtifactBuildError> {
    let text = std::str::from_utf8(raw)
        .map_err(|_| failure(checks, reason, "committed TOML is not UTF-8"))?;
    let value: Value = toml::from_str(text)
        .map_err(|error| failure(checks, reason, &format!("invalid TOML: {error}")))?;
    if value.is_table() {
        Ok(value)
    } else {
        Err(failure(checks, reason, "TOML root is not a table"))
    }
}

fn string_array(
    value: Option<&Value>,
    label: &str,
    checks: &[CandidateCheck],
) -> Result<Vec<String>, ContextArtifactBuildError> {
    value
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .map(|item| item.as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()
        })
        .ok_or_else(|| {
            failure(
                checks,
                "DRAFT_PAIR_MISMATCH",
                &format!("{label} is not a string array"),
            )
        })
}

fn selector_path_safe(selector: &str) -> bool {
    selector
        .split_once("::")
        .is_some_and(|(path, expression)| safe_path(path) && !expression.is_empty())
}

fn unique(values: &[String]) -> bool {
    values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn count_rows(document: &Value, key: &str) -> Option<i64> {
    document
        .get(key)
        .and_then(Value::as_array)
        .and_then(|rows| i64::try_from(rows.len()).ok())
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn integer(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_integer)
}

fn boolean(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn pass(checks: &mut Vec<CandidateCheck>, id: &str, detail: &str) {
    checks.push(CandidateCheck {
        id: id.to_owned(),
        status: "PASS",
        reason_code: None,
        detail: detail.to_owned(),
    });
}

fn require(
    condition: bool,
    checks: &mut Vec<CandidateCheck>,
    id: &str,
    reason: &str,
    detail: &str,
) -> Result<(), ContextArtifactBuildError> {
    if condition {
        pass(checks, id, detail);
        Ok(())
    } else {
        Err(failure(checks, reason, detail))
    }
}

fn failure(
    checks: &[CandidateCheck],
    reason: &str,
    message: &str,
) -> ContextArtifactBuildError {
    ContextArtifactBuildError::with_checks(reason, message, checks.to_vec())
}

fn map_git_without_checks(error: GitTreeError) -> ContextArtifactBuildError {
    ContextArtifactBuildError::new(error.reason(), error.message())
}

fn map_git(error: GitTreeError, checks: &[CandidateCheck]) -> ContextArtifactBuildError {
    failure(checks, error.reason(), error.message())
}
