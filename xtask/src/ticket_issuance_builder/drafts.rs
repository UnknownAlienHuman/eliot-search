//! Ticket/context draft-pair loading and fail-closed schema checks.

use toml::Value;

use crate::git_tree::GitTree;
use crate::ticket_planner::{
    CONTEXT_ALLOWED, CONTEXT_CANONICALIZATION_FIELDS, CONTEXT_CONTENT_FIELDS,
    REPOSITORY_NAME, TICKET_ALLOWED, TICKET_CONTEXT_FIELDS,
    TICKET_DELIVERABLES_FIELDS, TICKET_DEPENDENCIES_FIELDS,
    TICKET_LIMITS_FIELDS, TICKET_REPOSITORY_FENCE_FIELDS,
    TICKET_UNRESOLVED_IDENTITY_FIELDS, contract_pack_sources,
    exact_sha256_hex, expected_handoff_slots, expected_required_handoffs,
    line_limits_ok, safe_path, select_ceiling, unknown_fields,
};

use super::model::{Checks, DraftPair, RegistrySnapshot};
use super::util::{
    boolean, count_string, integer, string_array, strings_unique, table_keys,
    text, unique_row,
};

pub(super) fn load_draft_pair(
    tree: &GitTree,
    package: &str,
    registries: &RegistrySnapshot,
    checks: &mut Checks,
) -> Option<DraftPair> {
    let (ticket_manifest, context_manifest) = match (
        tree.load_toml("swarm/ticket-drafts/manifest.toml"),
        tree.load_toml("swarm/context-drafts/manifest.toml"),
    ) {
        (Ok((tickets, _)), Ok((contexts, _))) => (tickets, contexts),
        (Err(error), _) | (_, Err(error)) => {
            checks.fail("draft-manifests", "DRAFT_PAIR_MISSING", error.message());
            return None;
        }
    };

    let ticket_count = ticket_manifest
        .get("draft")
        .and_then(Value::as_array)
        .map(Vec::len);
    let context_count = context_manifest
        .get("draft")
        .and_then(Value::as_array)
        .map(Vec::len);
    let manifest_ok = integer(&ticket_manifest, "schema_version") == Some(2)
        && integer(&ticket_manifest, "ticket_draft_schema_version") == Some(2)
        && integer(&context_manifest, "schema_version") == Some(2)
        && integer(&context_manifest, "context_draft_schema_version") == Some(2)
        && ticket_count.and_then(|value| i64::try_from(value).ok())
            == integer(&ticket_manifest, "draft_count")
        && context_count.and_then(|value| i64::try_from(value).ok())
            == integer(&context_manifest, "draft_count");
    if manifest_ok {
        checks.pass(
            "draft-manifest-versions",
            "schema-v2 draft manifests are coherent",
        );
    } else {
        checks.fail(
            "draft-manifest-versions",
            "DRAFT_MANIFEST_MISMATCH",
            "draft manifest schema/count mismatch",
        );
    }

    let ticket_row = unique_row(&ticket_manifest, "draft", "package", package);
    let context_row = unique_row(&context_manifest, "draft", "package", package);
    let (Some(ticket_path), Some(context_path), Some(ceiling_class)) = (
        ticket_row.as_ref().and_then(|row| text(row, "path")),
        context_row.as_ref().and_then(|row| text(row, "path")),
        context_row
            .as_ref()
            .and_then(|row| text(row, "source_ceiling_class")),
    ) else {
        checks.fail(
            "draft-pair",
            "DRAFT_PAIR_MISSING",
            "ticket/context draft pair is missing or duplicate",
        );
        return None;
    };
    let ticket_path = ticket_path.to_owned();
    let context_path = context_path.to_owned();
    let ceiling_class = ceiling_class.to_owned();
    if !safe_path(&ticket_path) || !safe_path(&context_path) {
        checks.fail(
            "draft-paths",
            "DRAFT_PAIR_MISMATCH",
            "draft path is not repository-relative safe",
        );
        return None;
    }

    let (ticket_raw, ticket_entry) = match tree.read_bytes(&ticket_path) {
        Ok(value) => value,
        Err(error) => {
            checks.fail("draft-files", "DRAFT_PAIR_MISMATCH", error.message());
            return None;
        }
    };
    let (context_raw, context_entry) = match tree.read_bytes(&context_path) {
        Ok(value) => value,
        Err(error) => {
            checks.fail("draft-files", "DRAFT_PAIR_MISMATCH", error.message());
            return None;
        }
    };
    let ticket = match parse_toml(&ticket_raw) {
        Ok(value) => value,
        Err(detail) => {
            checks.fail("draft-files", "DRAFT_PAIR_MISMATCH", detail);
            return None;
        }
    };
    let context = match parse_toml(&context_raw) {
        Ok(value) => value,
        Err(detail) => {
            checks.fail("draft-files", "DRAFT_PAIR_MISMATCH", detail);
            return None;
        }
    };

    validate_keys(checks, "ticket-fields", &ticket, &TICKET_ALLOWED);
    validate_keys(checks, "context-fields", &context, &CONTEXT_ALLOWED);
    validate_section(
        checks,
        "ticket-unresolved_identity",
        &ticket,
        "unresolved_identity",
        &TICKET_UNRESOLVED_IDENTITY_FIELDS,
    );
    validate_section(
        checks,
        "ticket-repository_fence",
        &ticket,
        "repository_fence",
        &TICKET_REPOSITORY_FENCE_FIELDS,
    );
    validate_section(
        checks,
        "ticket-context",
        &ticket,
        "context",
        &TICKET_CONTEXT_FIELDS,
    );
    validate_section(
        checks,
        "ticket-dependencies",
        &ticket,
        "dependencies",
        &TICKET_DEPENDENCIES_FIELDS,
    );
    validate_section(
        checks,
        "ticket-limits",
        &ticket,
        "limits",
        &TICKET_LIMITS_FIELDS,
    );
    validate_section(
        checks,
        "ticket-deliverables",
        &ticket,
        "deliverables",
        &TICKET_DELIVERABLES_FIELDS,
    );
    validate_section(
        checks,
        "context-canonicalization",
        &context,
        "canonicalization",
        &CONTEXT_CANONICALIZATION_FIELDS,
    );
    validate_section(
        checks,
        "context-content",
        &context,
        "content",
        &CONTEXT_CONTENT_FIELDS,
    );

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
        && ticket
            .get("context")
            .and_then(Value::as_table)
            .and_then(|table| table.get("context_draft"))
            .and_then(Value::as_str)
            == Some(context_path.as_str());
    if identity_ok {
        checks.pass(
            "draft-identity",
            "schema-v2 ticket/context pair binds P00/W0 package",
        );
    } else {
        checks.fail(
            "draft-identity",
            "DRAFT_PAIR_MISMATCH",
            "ticket/context package, schema or stage mismatch",
        );
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
    if nonclaimable {
        checks.pass(
            "draft-authority",
            "both drafts are non-claimable and non-authorizing",
        );
    } else {
        checks.fail(
            "draft-authority",
            "DRAFT_BECAME_CLAIMABLE",
            "draft authority flags are unsafe",
        );
    }

    validate_unresolved(&ticket, &context, checks);
    validate_fence(&ticket, registries.package_row.as_ref(), checks);
    validate_dependencies(&ticket, package, checks);

    let Some(content) = context.get("content").and_then(Value::as_table) else {
        checks.fail(
            "context-arrays",
            "DRAFT_PAIR_MISMATCH",
            "context content table is missing",
        );
        return None;
    };
    let arrays = (
        string_array(content.get("source_files")),
        string_array(content.get("registry_fragments")),
        string_array(content.get("accepted_handoff_slots")),
        string_array(content.get("forbidden_paths")),
        string_array(content.get("required_unavailable_checks")),
    );
    let (Some(sources), Some(selectors), Some(slots), Some(forbidden), Some(unavailable)) = arrays else {
        checks.fail(
            "context-arrays",
            "DRAFT_PAIR_MISMATCH",
            "context arrays must contain only strings",
        );
        return None;
    };

    validate_context_counts(
        &context,
        &context_manifest,
        package,
        &ceiling_class,
        &sources,
        &selectors,
        &slots,
        &unavailable,
        checks,
    );
    if slots == expected_handoff_slots(package) {
        checks.pass("context-handoff-slots", "exact accepted-handoff slots");
    } else {
        checks.fail(
            "context-handoff-slots",
            "DRAFT_PAIR_MISMATCH",
            "accepted-handoff slots differ from P00 dependency topology",
        );
    }
    validate_canonicalization(&context, &forbidden, &unavailable, checks);
    if ceiling_class == "P00_EXACT_CONTRACT_PACK" {
        validate_exact_pack(tree, package, &sources, checks);
    }

    Some(DraftPair {
        ticket_path,
        context_path,
        ticket,
        context,
        sources,
        selectors,
        handoff_slots: slots,
        unavailable_checks: unavailable,
        source_ceiling_class: ceiling_class,
        ticket_blob: tree.blob_identity(&ticket_entry),
        ticket_sha256: exact_sha256_hex(&ticket_raw),
        context_blob: tree.blob_identity(&context_entry),
        context_sha256: exact_sha256_hex(&context_raw),
    })
}

fn parse_toml(raw: &[u8]) -> Result<Value, String> {
    let text = std::str::from_utf8(raw)
        .map_err(|error| format!("draft is not strict UTF-8: {error}"))?;
    toml::from_str(text).map_err(|error| format!("invalid draft TOML: {error}"))
}

fn validate_keys(
    checks: &mut Checks,
    id: &str,
    value: &Value,
    allowed: &[&str],
) {
    let keys = table_keys(value);
    let unknown = unknown_fields(&keys, allowed);
    if unknown.is_empty() {
        checks.pass(id, "field set is closed");
    } else {
        checks.fail(
            id,
            "DRAFT_UNKNOWN_FIELD",
            format!("unknown fields: {}", unknown.join(",")),
        );
    }
}

fn validate_section(
    checks: &mut Checks,
    id: &str,
    document: &Value,
    section: &str,
    allowed: &[&str],
) {
    let Some(table) = document.get(section) else {
        checks.fail(
            id,
            "DRAFT_PAIR_MISMATCH",
            format!("missing [{section}] table"),
        );
        return;
    };
    if !table.is_table() {
        checks.fail(
            id,
            "DRAFT_PAIR_MISMATCH",
            format!("missing [{section}] table"),
        );
        return;
    }
    validate_keys(checks, &format!("{id}-fields"), table, allowed);
}

fn validate_unresolved(ticket: &Value, context: &Value, checks: &mut Checks) {
    let unresolved = ticket.get("unresolved_identity").and_then(Value::as_table);
    let expected = [
        ("ticket_id", "UNASSIGNED"),
        ("writer", "UNASSIGNED"),
        ("reviewer", "UNASSIGNED"),
        ("issued_at", ""),
        ("base_commit", "UNSELECTED"),
        ("branch_or_worktree", "UNSELECTED"),
        ("ticket_signed_payload_sha256", "UNAVAILABLE"),
        ("ticket_exact_record_file_sha256", "UNAVAILABLE"),
        ("integration_signature_ref", ""),
    ];
    let unresolved_ok = unresolved.is_some_and(|table| {
        expected.iter().all(|(key, value)| {
            table.get(*key).and_then(Value::as_str) == Some(*value)
        })
    }) && text(context, "base_commit") == Some("UNSELECTED")
        && text(context, "materialized_context_manifest_ref") == Some("UNAVAILABLE")
        && text(context, "materialized_context_record_sha256") == Some("UNAVAILABLE")
        && text(context, "materialized_context_artifact_ref") == Some("UNAVAILABLE")
        && text(context, "materialized_context_artifact_sha256") == Some("UNAVAILABLE");
    if unresolved_ok {
        checks.pass(
            "draft-unresolved",
            "ticket, manifest and artifact identities remain unresolved",
        );
    } else {
        checks.fail(
            "draft-unresolved",
            "DRAFT_IDENTITY_PREMATURELY_RESOLVED",
            "draft contains premature issuance identity",
        );
    }
}

fn validate_fence(
    ticket: &Value,
    package_row: Option<&Value>,
    checks: &mut Checks,
) {
    let fence = ticket.get("repository_fence").and_then(Value::as_table);
    let context = ticket.get("context").and_then(Value::as_table);
    let limits = ticket.get("limits").and_then(Value::as_table);
    let package_path = package_row.and_then(|row| text(row, "path"));
    let registry_soft = package_row.and_then(|row| integer(row, "soft_src_line_target"));
    let coherent_limits = match (registry_soft, limits) {
        (Some(registry_soft), Some(limits)) => {
            let ticket_soft = limits.get("soft_src_lines").and_then(Value::as_integer);
            let split = limits
                .get("split_review_total_lines")
                .and_then(Value::as_integer);
            let hard = limits.get("hard_total_lines").and_then(Value::as_integer);
            match (ticket_soft, split, hard) {
                (Some(ticket_soft), Some(split), Some(hard)) => {
                    line_limits_ok(registry_soft, ticket_soft, split, hard)
                }
                _ => false,
            }
        }
        _ => false,
    };
    let expected_scope = package_path.map(|path| format!("{path}/**"));
    let fence_ok = fence.is_some_and(|table| {
        table.get("repository").and_then(Value::as_str) == Some(REPOSITORY_NAME)
            && table.get("write_scope").and_then(Value::as_str)
                == expected_scope.as_deref()
            && table.get("feature_profile").and_then(Value::as_str)
                == Some("P00_FOUNDATION")
            && table.get("package_registry_path").and_then(Value::as_str)
                == Some("swarm/crates.toml")
            && table.get("function_registry_path").and_then(Value::as_str)
                == Some("swarm/function-packets.toml")
            && table.get("stage_registry_path").and_then(Value::as_str)
                == Some("swarm/stages.toml")
            && table.get("launch_state_path").and_then(Value::as_str)
                == Some("swarm/launch-state.toml")
            && table.get("registry_digests").and_then(Value::as_str)
                == Some("UNRESOLVED_AT_ISSUANCE")
    }) && context.is_some_and(|table| {
        table.get("writer_visible_artifact_count").and_then(Value::as_integer)
            == Some(1)
            && table.get("architecture_access").and_then(Value::as_str)
                == Some("exception-only")
    }) && limits.is_some_and(|table| {
        table.get("one_active_writer").and_then(Value::as_bool) == Some(true)
    }) && coherent_limits;
    if fence_ok {
        checks.pass(
            "draft-repository-fence",
            "repository, scope, context and line limits are coherent",
        );
    } else {
        checks.fail(
            "draft-repository-fence",
            "DRAFT_PAIR_MISMATCH",
            "repository fence, context or line limits mismatch",
        );
    }
}

fn validate_dependencies(ticket: &Value, package: &str, checks: &mut Checks) {
    let dependencies = ticket.get("dependencies").and_then(Value::as_table);
    let required = dependencies
        .and_then(|table| string_array(table.get("required_handoff_packages")))
        .unwrap_or_default();
    let accepted = dependencies
        .and_then(|table| string_array(table.get("accepted_handoff_refs")))
        .unwrap_or_default();
    let expected: Vec<String> = expected_required_handoffs(package)
        .iter()
        .map(|value| (*value).to_owned())
        .collect();
    let mut ok = dependencies.is_some()
        && required == expected
        && accepted.is_empty()
        && dependencies
            .and_then(|table| table.get("status"))
            .and_then(Value::as_str)
            == Some(if package == "search-contracts" {
                "NOT_REQUIRED"
            } else {
                "UNAVAILABLE"
            });
    if package != "search-contracts" {
        ok = ok
            && dependencies
                .and_then(|table| table.get("required_contract_commit"))
                .and_then(Value::as_str)
                == Some("UNSELECTED")
            && dependencies
                .and_then(|table| {
                    table.get("required_contract_api_schema_digest")
                })
                .and_then(Value::as_str)
                == Some("UNAVAILABLE");
    }
    if ok {
        checks.pass(
            "ticket-dependencies",
            "draft dependencies are exact and unresolved",
        );
    } else {
        checks.fail(
            "ticket-dependencies",
            "DRAFT_PAIR_MISMATCH",
            "ticket dependency set or sentinels mismatch",
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_context_counts(
    context: &Value,
    manifest: &Value,
    package: &str,
    ceiling_class: &str,
    sources: &[String],
    selectors: &[String],
    slots: &[String],
    unavailable: &[String],
    checks: &mut Checks,
) {
    let ordinary = integer(manifest, "ordinary_static_source_file_ceiling");
    let exact = integer(manifest, "p00_exact_contract_pack_source_file_ceiling");
    let exceptions = string_array(
        manifest.get("p00_exact_contract_pack_exception_packages"),
    )
    .unwrap_or_default();
    let exception_refs: Vec<&str> = exceptions.iter().map(String::as_str).collect();
    let (ceiling, class_ok) = match (ordinary, exact) {
        (Some(ordinary), Some(exact)) => {
            select_ceiling(ceiling_class, package, &exception_refs, ordinary, exact)
        }
        _ => (0, false),
    };
    let counts_ok = usize::try_from(ceiling).is_ok_and(|value| sources.len() <= value)
        && integer(context, "source_file_count")
            == i64::try_from(sources.len()).ok()
        && integer(context, "registry_fragment_count")
            == i64::try_from(selectors.len()).ok()
        && integer(manifest, "max_registry_fragments_per_context")
            .is_some_and(|value| {
                usize::try_from(value).is_ok_and(|value| selectors.len() <= value)
            })
        && integer(context, "accepted_handoff_slot_count")
            == i64::try_from(slots.len()).ok()
        && integer(manifest, "max_accepted_handoff_slots_per_context")
            .is_some_and(|value| {
                usize::try_from(value).is_ok_and(|value| slots.len() <= value)
            })
        && integer(context, "writer_visible_artifact_count") == Some(1)
        && strings_unique(sources)
        && strings_unique(selectors)
        && strings_unique(slots)
        && strings_unique(unavailable)
        && class_ok;
    if counts_ok {
        checks.pass(
            "context-counts",
            format!("context uses manifest-owned {ceiling_class} ceilings"),
        );
    } else {
        checks.fail(
            "context-counts",
            "CONTEXT_BUDGET_EXCEEDED",
            "context counts, uniqueness or manifest-owned ceilings mismatch",
        );
    }
}

fn validate_canonicalization(
    context: &Value,
    forbidden: &[String],
    unavailable: &[String],
    checks: &mut Checks,
) {
    let canonical = context.get("canonicalization").and_then(Value::as_table);
    let ok = canonical.is_some_and(|table| {
        table.get("encoding").and_then(Value::as_str) == Some("UTF-8")
            && table.get("line_endings").and_then(Value::as_str) == Some("LF")
            && table.get("preserve_declared_order").and_then(Value::as_bool)
                == Some(true)
            && table.get("record_source_sha256").and_then(Value::as_bool)
                == Some(true)
            && table.get("record_fragment_sha256").and_then(Value::as_bool)
                == Some(true)
    }) && text(context, "materialization_mode")
        == Some("canonical_concatenated_bundle")
        && !unavailable.is_empty()
        && forbidden.iter().any(|path| path == "docs/architecture/**");
    if ok {
        checks.pass(
            "context-canonicalization",
            "context canonicalization and unavailable checks are explicit",
        );
    } else {
        checks.fail(
            "context-canonicalization",
            "DRAFT_PAIR_MISMATCH",
            "context canonicalization, forbidden paths or unavailable checks mismatch",
        );
    }
}

fn validate_exact_pack(
    tree: &GitTree,
    package: &str,
    sources: &[String],
    checks: &mut Checks,
) {
    let required = match tree.load_toml("docs/contracts/p00/manifest.toml") {
        Ok((manifest, _)) => string_array(manifest.get("required_files")),
        Err(error) => {
            checks.fail("context-exact-pack", "DRAFT_MANIFEST_MISMATCH", error.message());
            return;
        }
    };
    let Some(required) = required else {
        checks.fail(
            "context-exact-pack",
            "DRAFT_MANIFEST_MISMATCH",
            "P00 contract manifest required_files is not canonical",
        );
        return;
    };
    let refs: Vec<&str> = required.iter().map(String::as_str).collect();
    match contract_pack_sources(package, &refs) {
        Ok(expected) if expected == sources => checks.pass(
            "context-exact-pack",
            "search-contracts source list equals the manifest-closed exact pack",
        ),
        _ => checks.fail(
            "context-exact-pack",
            "DRAFT_MANIFEST_MISMATCH",
            "search-contracts source list differs from exact P00 pack",
        ),
    }
}
