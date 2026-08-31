from .core import *  # noqa: F401,F403
from .drafts import load_draft_pair
from .context import validate_context, validate_registries, validate_control_schema
from .control import (
    selection_state,
    validate_root_metadata_and_package_state,
    validate_workflows,
    validate_handoffs,
    validate_output,
    choose_decision,
)


def build_plan(
    args: argparse.Namespace,
) -> tuple[dict[str, Any], Path | None]:
    root = Path(args.root).resolve()
    checks = Checks()
    state = selection_state(
        args.base_commit, args.writer, args.reviewer, checks
    )

    selected_for_view = args.base_commit if args.base_commit is not None else None
    try:
        view = GitView(root, selected_for_view)
        checks.pass_(
            "git-view",
            f"all repository inputs read from immutable commit {view.tagged_commit}",
        )
    except PlannerFailure as exc:
        if exc.reason_code != "BASE_COMMIT_INVALID":
            raise
        checks.fail("base-commit", exc.reason_code, exc.message)
        view = GitView(root, None)
        checks.pass_(
            "git-view-fallback",
            f"invalid selection inspected against immutable HEAD {view.tagged_commit}",
        )

    launch, package_row, function_row, stage_row = validate_registries(
        view, args.package, checks
    )
    pair = load_draft_pair(
        view, args.package, package_row, checks
    )
    sources = (
        validate_context(view, pair, args.package, checks) if pair else []
    )
    validate_control_schema(view, launch, checks)
    validate_root_metadata_and_package_state(view, args.package, checks)
    validate_workflows(view, checks)
    handoffs = (
        validate_handoffs(
            view, pair, args.accepted_handoff, checks
        )
        if pair
        else []
    )

    classification = "UNKNOWN"
    if (
        isinstance(launch.get("authorized_packages"), list)
        and launch["authorized_packages"].count(args.package) == 1
    ):
        classification = "AUTHORIZED"
    elif (
        isinstance(launch.get("conditional_packages"), list)
        and launch["conditional_packages"].count(args.package) == 1
    ):
        classification = "CONDITIONAL"
    if (
        pair
        and pair.ticket.get("launch_class") == classification
        and classification in {"AUTHORIZED", "CONDITIONAL"}
    ):
        checks.pass_(
            "launch-class",
            f"draft and launch classification agree: {classification}",
        )
    else:
        checks.fail(
            "launch-class",
            "PACKAGE_STAGE_MISMATCH",
            "draft and launch classification differ",
        )

    output_target = validate_output(root, args.output, checks)
    decision = choose_decision(state, checks.reasons)
    package_path = package_row.get("path", "") if package_row else ""
    package_wave = package_row.get("wave", -1) if package_row else -1
    scope = function_row.get("write_scope", "") if function_row else ""

    plan: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "record_kind": RECORD_KIND,
        "status": STATUS,
        "repository": {
            "name": REPOSITORY_NAME,
            "view_commit": view.tagged_commit,
            "object_format": view.object_format,
            "working_tree_used_as_input": False,
        },
        "package": {
            "name": args.package,
            "path": package_path,
            "wave": package_wave,
            "write_scope": scope,
            "source_ceiling_class": (
                pair.source_ceiling_class if pair else ""
            ),
        },
        "stage": {
            "id": pair.ticket.get("stage", "UNKNOWN")
            if pair
            else "UNKNOWN",
            "phase": pair.ticket.get("phase", "UNKNOWN")
            if pair
            else "UNKNOWN",
            "active_stage": launch.get("active_stage", "UNKNOWN"),
            "active_wave": launch.get("active_wave", -1),
            "registry_wave": stage_row.get("wave", -1)
            if stage_row
            else -1,
        },
        "launch": {
            "classification": classification,
            "classification_recognized": classification
            in {"AUTHORIZED", "CONDITIONAL"},
            "conditional_requirements": launch.get(
                "conditional_activation", {}
            ).get(args.package, {})
            if isinstance(launch.get("conditional_activation"), dict)
            else {},
        },
        "selection": {
            "state": state,
            "base_commit": args.base_commit or "",
            "writer": args.writer or "",
            "reviewer": args.reviewer or "",
        },
        "drafts": {
            "ticket_path": pair.ticket_path if pair else "",
            "ticket_git_blob_id": pair.ticket_blob if pair else "",
            "ticket_exact_sha256": pair.ticket_sha256 if pair else "",
            "context_path": pair.context_path if pair else "",
            "context_git_blob_id": pair.context_blob if pair else "",
            "context_exact_sha256": pair.context_sha256 if pair else "",
            "sources": sources,
            "registry_selectors": list(pair.selectors) if pair else [],
            "accepted_handoff_slots": list(pair.handoff_slots)
            if pair
            else [],
            "unavailable_checks": list(pair.unavailable_checks)
            if pair
            else [],
        },
        "prerequisites": {"accepted_handoffs": handoffs},
        "checks": checks.items,
        "decision": decision,
        "reason_codes": checks.reasons,
        "mutations": [],
        "authorizes_context_materialization": False,
        "authorizes_ticket_issuance": False,
        "creates_writer_lease": False,
        "authorizes_implementation": False,
        "publishes_package_handoff": False,
        "advances_launch_state": False,
    }
    plan["plan_sha256"] = plan_digest(plan)
    return plan, output_target


def write_plan(plan: Mapping[str, Any], target: Path | None) -> None:
    encoded = canonical_json_bytes(plan)
    if len(encoded) > 262_144:
        raise PlannerFailure(
            "OUTPUT_WRITE_FAILED",
            "canonical plan exceeds 262144-byte ceiling",
        )
    if target is None:
        sys.stdout.buffer.write(encoded)
        return
    temporary = target.with_name(target.name + f".tmp-{os.getpid()}")
    try:
        target.parent.mkdir(parents=True, exist_ok=True)
        temporary.write_bytes(encoded)
        os.replace(temporary, target)
    except OSError as exc:
        try:
            temporary.unlink(missing_ok=True)
        except OSError:
            pass
        raise PlannerFailure(
            "OUTPUT_WRITE_FAILED",
            f"unable to write advisory artifact: {exc}",
        ) from exc
