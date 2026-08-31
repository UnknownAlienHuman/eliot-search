from .core import *  # noqa: F401,F403


def load_draft_pair(
    view: GitView, package: str, package_row: Mapping[str, Any] | None, checks: Checks
) -> DraftPair | None:
    try:
        ticket_manifest, _ = view.load_toml(
            "swarm/ticket-drafts/manifest.toml"
        )
        context_manifest, _ = view.load_toml(
            "swarm/context-drafts/manifest.toml"
        )
    except PlannerFailure as exc:
        checks.fail("draft-manifests", "DRAFT_PAIR_MISSING", exc.message)
        return None

    manifest_ok = (
        ticket_manifest.get("schema_version") == 2
        and ticket_manifest.get("ticket_draft_schema_version") == 2
        and context_manifest.get("schema_version") == 2
        and context_manifest.get("context_draft_schema_version") == 2
        and ticket_manifest.get("draft_count")
        == len(ticket_manifest.get("draft", []))
        and context_manifest.get("draft_count")
        == len(context_manifest.get("draft", []))
    )
    if manifest_ok:
        checks.pass_("draft-manifest-versions", "schema-v2 draft manifests are coherent")
    else:
        checks.fail(
            "draft-manifest-versions",
            "DRAFT_MANIFEST_MISMATCH",
            "draft manifest schema/count mismatch",
        )

    ticket_row = one_table(ticket_manifest.get("draft"), "package", package)
    context_row = one_table(context_manifest.get("draft"), "package", package)
    ticket_path = ticket_row.get("path") if ticket_row else None
    context_path = context_row.get("path") if context_row else None
    ceiling_class = (
        context_row.get("source_ceiling_class") if context_row else None
    )
    if (
        not isinstance(ticket_path, str)
        or not isinstance(context_path, str)
        or not isinstance(ceiling_class, str)
    ):
        checks.fail(
            "draft-pair",
            "DRAFT_PAIR_MISSING",
            "ticket/context draft pair is missing or duplicate",
        )
        return None
    if not safe_path(ticket_path) or not safe_path(context_path):
        checks.fail(
            "draft-paths",
            "DRAFT_PAIR_MISMATCH",
            "draft path is not repository-relative safe",
        )
        return None
    try:
        ticket_raw, ticket_entry = view.read_bytes(ticket_path)
        context_raw, context_entry = view.read_bytes(context_path)
        ticket = tomllib.loads(ticket_raw.decode("utf-8", "strict"))
        context = tomllib.loads(context_raw.decode("utf-8", "strict"))
    except (PlannerFailure, UnicodeDecodeError, tomllib.TOMLDecodeError) as exc:
        detail = exc.message if isinstance(exc, PlannerFailure) else str(exc)
        checks.fail("draft-files", "DRAFT_PAIR_MISMATCH", detail)
        return None

    validate_keys(checks, "ticket-fields", ticket, TICKET_ALLOWED)
    validate_keys(checks, "context-fields", context, CONTEXT_ALLOWED)
    for section, allowed in TICKET_SECTIONS.items():
        table = ticket.get(section)
        if not isinstance(table, dict):
            checks.fail(
                f"ticket-{section}",
                "DRAFT_PAIR_MISMATCH",
                f"missing [{section}] table",
            )
        else:
            validate_keys(
                checks, f"ticket-{section}-fields", table, allowed
            )
    for section, allowed in CONTEXT_SECTIONS.items():
        table = context.get(section)
        if not isinstance(table, dict):
            checks.fail(
                f"context-{section}",
                "DRAFT_PAIR_MISMATCH",
                f"missing [{section}] table",
            )
        else:
            validate_keys(
                checks, f"context-{section}-fields", table, allowed
            )

    identity_ok = (
        ticket.get("schema_version") == 2
        and context.get("schema_version") == 2
        and ticket.get("package") == package
        and context.get("package") == package
        and ticket.get("stage") == "W0"
        and context.get("stage") == "W0"
        and ticket.get("phase") == "P00"
        and context.get("phase") == "P00"
        and ticket.get("wave") == 0
        and context.get("wave") == 0
        and isinstance(ticket.get("context"), dict)
        and ticket["context"].get("context_draft") == context_path
    )
    if identity_ok:
        checks.pass_(
            "draft-identity", "schema-v2 ticket/context pair binds P00/W0 package"
        )
    else:
        checks.fail(
            "draft-identity",
            "DRAFT_PAIR_MISMATCH",
            "ticket/context package, schema or stage mismatch",
        )

    nonclaimable = (
        ticket.get("record_kind") == "assignment_ticket_draft"
        and ticket.get("status") == "DRAFT_ONLY_NOT_ISSUED"
        and ticket.get("claimable") is False
        and ticket.get("authorizes_implementation") is False
        and ticket.get("creates_lease") is False
        and ticket.get("may_be_writer_acknowledged") is False
        and context.get("record_kind") == "writer_context_draft"
        and context.get("status") == "UNMATERIALIZED_DRAFT"
        and context.get("claimable") is False
        and context.get("authorizes_implementation") is False
    )
    if nonclaimable:
        checks.pass_(
            "draft-authority",
            "both drafts are non-claimable and non-authorizing",
        )
    else:
        checks.fail(
            "draft-authority",
            "DRAFT_BECAME_CLAIMABLE",
            "draft authority flags are unsafe",
        )

    unresolved = (
        ticket.get("unresolved_identity")
        if isinstance(ticket.get("unresolved_identity"), dict)
        else {}
    )
    unresolved_expected = {
        "ticket_id": "UNASSIGNED",
        "writer": "UNASSIGNED",
        "reviewer": "UNASSIGNED",
        "issued_at": "",
        "base_commit": "UNSELECTED",
        "branch_or_worktree": "UNSELECTED",
        "ticket_signed_payload_sha256": "UNAVAILABLE",
        "ticket_exact_record_file_sha256": "UNAVAILABLE",
        "integration_signature_ref": "",
    }
    unresolved_ok = all(
        unresolved.get(key) == expected
        for key, expected in unresolved_expected.items()
    )
    unresolved_ok = (
        unresolved_ok
        and context.get("base_commit") == "UNSELECTED"
        and context.get("materialized_context_manifest_ref") == "UNAVAILABLE"
        and context.get("materialized_context_record_sha256") == "UNAVAILABLE"
        and context.get("materialized_context_artifact_ref") == "UNAVAILABLE"
        and context.get("materialized_context_artifact_sha256") == "UNAVAILABLE"
    )
    if unresolved_ok:
        checks.pass_(
            "draft-unresolved",
            "ticket, manifest and artifact identities remain unresolved",
        )
    else:
        checks.fail(
            "draft-unresolved",
            "DRAFT_IDENTITY_PREMATURELY_RESOLVED",
            "draft contains premature issuance identity",
        )

    repository_fence = (
        ticket.get("repository_fence")
        if isinstance(ticket.get("repository_fence"), dict)
        else {}
    )
    ticket_context = (
        ticket.get("context") if isinstance(ticket.get("context"), dict) else {}
    )
    limits = ticket.get("limits") if isinstance(ticket.get("limits"), dict) else {}
    path = package_row.get("path") if package_row else None
    registry_soft_target = (
        package_row.get("soft_src_line_target") if package_row else None
    )
    ticket_soft_limit = limits.get("soft_src_lines")
    split_review_limit = limits.get("split_review_total_lines")
    hard_limit = limits.get("hard_total_lines")
    line_limits_ok = (
        isinstance(registry_soft_target, int)
        and isinstance(ticket_soft_limit, int)
        and isinstance(split_review_limit, int)
        and isinstance(hard_limit, int)
        and 0 < registry_soft_target <= ticket_soft_limit
        and ticket_soft_limit <= split_review_limit <= hard_limit
        and split_review_limit == 8500
        and hard_limit == 10000
    )
    fence_ok = (
        repository_fence.get("repository") == REPOSITORY_NAME
        and repository_fence.get("write_scope") == f"{path}/**"
        and repository_fence.get("feature_profile") == "P00_FOUNDATION"
        and repository_fence.get("package_registry_path") == "swarm/crates.toml"
        and repository_fence.get("function_registry_path")
        == "swarm/function-packets.toml"
        and repository_fence.get("stage_registry_path") == "swarm/stages.toml"
        and repository_fence.get("launch_state_path") == "swarm/launch-state.toml"
        and repository_fence.get("registry_digests")
        == "UNRESOLVED_AT_ISSUANCE"
        and ticket_context.get("writer_visible_artifact_count") == 1
        and ticket_context.get("architecture_access") == "exception-only"
        and line_limits_ok
        and limits.get("one_active_writer") is True
    )
    if fence_ok:
        checks.pass_(
            "draft-repository-fence",
            "repository, scope, context and line limits are coherent",
        )
    else:
        checks.fail(
            "draft-repository-fence",
            "DRAFT_PAIR_MISMATCH",
            "repository fence, context or line limits mismatch",
        )

    dependencies = (
        ticket.get("dependencies")
        if isinstance(ticket.get("dependencies"), dict)
        else {}
    )
    try:
        required_handoffs = as_string_array(
            dependencies.get("required_handoff_packages"),
            "required_handoff_packages",
        )
        accepted_refs = as_string_array(
            dependencies.get("accepted_handoff_refs"),
            "accepted_handoff_refs",
        )
    except PlannerFailure as exc:
        checks.fail("ticket-dependencies", exc.reason_code, exc.message)
        required_handoffs = ()
        accepted_refs = ()
    expected_handoffs = () if package == "search-contracts" else ("search-contracts",)
    dependency_ok = (
        required_handoffs == expected_handoffs
        and accepted_refs == ()
        and (
            dependencies.get("status") == "NOT_REQUIRED"
            if package == "search-contracts"
            else dependencies.get("status") == "UNAVAILABLE"
        )
    )
    if package != "search-contracts":
        dependency_ok = (
            dependency_ok
            and dependencies.get("required_contract_commit") == "UNSELECTED"
            and dependencies.get("required_contract_api_schema_digest")
            == "UNAVAILABLE"
        )
    if dependency_ok:
        checks.pass_(
            "ticket-dependencies",
            "draft dependencies are exact and unresolved",
        )
    else:
        checks.fail(
            "ticket-dependencies",
            "DRAFT_PAIR_MISMATCH",
            "ticket dependency set or sentinels mismatch",
        )

    content = (
        context.get("content") if isinstance(context.get("content"), dict) else {}
    )
    try:
        sources = as_string_array(content.get("source_files"), "source_files")
        selectors = as_string_array(
            content.get("registry_fragments"), "registry_fragments"
        )
        slots = as_string_array(
            content.get("accepted_handoff_slots"), "accepted_handoff_slots"
        )
        forbidden = as_string_array(
            content.get("forbidden_paths"), "forbidden_paths"
        )
        unavailable = as_string_array(
            content.get("required_unavailable_checks"),
            "required_unavailable_checks",
        )
    except PlannerFailure as exc:
        checks.fail("context-arrays", exc.reason_code, exc.message)
        return None

    ordinary_ceiling = context_manifest.get(
        "ordinary_static_source_file_ceiling"
    )
    exact_ceiling = context_manifest.get(
        "p00_exact_contract_pack_source_file_ceiling"
    )
    exceptions = context_manifest.get(
        "p00_exact_contract_pack_exception_packages"
    )
    fragment_ceiling = context_manifest.get("max_registry_fragments_per_context")
    handoff_ceiling = context_manifest.get(
        "max_accepted_handoff_slots_per_context"
    )
    ceiling = ordinary_ceiling
    class_ok = ceiling_class == "ORDINARY"
    if ceiling_class == "P00_EXACT_CONTRACT_PACK":
        ceiling = exact_ceiling
        class_ok = (
            isinstance(exceptions, list)
            and exceptions.count(package) == 1
            and package == "search-contracts"
        )
    counts_ok = (
        isinstance(ceiling, int)
        and context.get("source_file_count") == len(sources)
        and len(sources) <= ceiling
        and context.get("registry_fragment_count") == len(selectors)
        and isinstance(fragment_ceiling, int)
        and len(selectors) <= fragment_ceiling
        and context.get("accepted_handoff_slot_count") == len(slots)
        and isinstance(handoff_ceiling, int)
        and len(slots) <= handoff_ceiling
        and context.get("writer_visible_artifact_count") == 1
        and len(set(sources)) == len(sources)
        and len(set(selectors)) == len(selectors)
        and len(set(slots)) == len(slots)
        and len(set(unavailable)) == len(unavailable)
        and class_ok
    )
    if counts_ok:
        checks.pass_(
            "context-counts",
            f"context uses manifest-owned {ceiling_class} ceilings",
        )
    else:
        checks.fail(
            "context-counts",
            "CONTEXT_BUDGET_EXCEEDED",
            "context counts, uniqueness or manifest-owned ceilings mismatch",
        )

    expected_slots = (
        ()
        if package == "search-contracts"
        else ("search-contracts::accepted_package_and_api_handoff",)
    )
    if slots == expected_slots:
        checks.pass_("context-handoff-slots", "exact accepted-handoff slots")
    else:
        checks.fail(
            "context-handoff-slots",
            "DRAFT_PAIR_MISMATCH",
            "accepted-handoff slots differ from P00 dependency topology",
        )

    canonicalization = (
        context.get("canonicalization")
        if isinstance(context.get("canonicalization"), dict)
        else {}
    )
    canonical_ok = (
        canonicalization.get("encoding") == "UTF-8"
        and canonicalization.get("line_endings") == "LF"
        and canonicalization.get("preserve_declared_order") is True
        and canonicalization.get("record_source_sha256") is True
        and canonicalization.get("record_fragment_sha256") is True
        and context.get("materialization_mode") == "canonical_concatenated_bundle"
        and bool(unavailable)
        and "docs/architecture/**" in forbidden
    )
    if canonical_ok:
        checks.pass_(
            "context-canonicalization",
            "context canonicalization and unavailable checks are explicit",
        )
    else:
        checks.fail(
            "context-canonicalization",
            "DRAFT_PAIR_MISMATCH",
            "context canonicalization, forbidden paths or unavailable checks mismatch",
        )

    if ceiling_class == "P00_EXACT_CONTRACT_PACK":
        try:
            expected_sources = expected_contract_pack_sources(view, package)
        except PlannerFailure as exc:
            checks.fail(
                "context-exact-pack",
                exc.reason_code,
                exc.message,
            )
        else:
            if sources == expected_sources:
                checks.pass_(
                    "context-exact-pack",
                    "search-contracts source list equals the manifest-closed exact pack",
                )
            else:
                checks.fail(
                    "context-exact-pack",
                    "DRAFT_MANIFEST_MISMATCH",
                    "search-contracts source list differs from exact P00 pack",
                )

    return DraftPair(
        ticket_path=ticket_path,
        context_path=context_path,
        ticket=ticket,
        context=context,
        sources=sources,
        selectors=selectors,
        handoff_slots=slots,
        unavailable_checks=unavailable,
        source_ceiling_class=ceiling_class,
        ticket_blob=view.blob_identity(ticket_entry),
        ticket_sha256=exact_sha256(ticket_raw),
        context_blob=view.blob_identity(context_entry),
        context_sha256=exact_sha256(context_raw),
    )
