from .core import *  # noqa: F401,F403


def resolve_selector(
    view: GitView, selector: str, package: str
) -> tuple[str, str]:
    if "::" not in selector:
        return "UNSUPPORTED", "missing :: separator"
    path, expression = selector.split("::", 1)
    allowed_paths = {
        "swarm/crates.toml",
        "swarm/function-packets.toml",
        "swarm/stages.toml",
        "swarm/launch-state.toml",
    }
    if path not in allowed_paths:
        return "UNSUPPORTED", "registry path is not in the closed selector set"
    try:
        document, _ = view.load_toml(path)
    except PlannerFailure:
        return "NOT_UNIQUE", "registry path is missing or invalid"

    match = re.fullmatch(
        r"package\[name=([a-z][a-z0-9-]*)\]", expression
    )
    if match:
        if path != "swarm/crates.toml" or match.group(1) != package:
            return "UNSUPPORTED", "package selector path or identity mismatch"
        row = one_table(document.get("package"), "name", package)
        return ("OK", "one package row") if row is not None else (
            "NOT_UNIQUE",
            "package selector did not resolve exactly once",
        )

    match = re.fullmatch(
        r"foundation\[package=([a-z][a-z0-9-]*)\]", expression
    )
    if match:
        if path != "swarm/function-packets.toml" or match.group(1) != package:
            return "UNSUPPORTED", "foundation selector path or identity mismatch"
        row = one_table(document.get("foundation"), "package", package)
        return ("OK", "one foundation row") if row is not None else (
            "NOT_UNIQUE",
            "foundation selector did not resolve exactly once",
        )

    match = re.fullmatch(r"stage\[id=(W(?:10|[0-9]))\]", expression)
    if match:
        if path != "swarm/stages.toml" or match.group(1) != "W0":
            return "UNSUPPORTED", "stage selector path or stage mismatch"
        row = one_table(document.get("stage"), "id", "W0")
        if row is None:
            return "NOT_UNIQUE", "stage selector did not resolve exactly once"
        packages = row.get("packages")
        if not isinstance(packages, list) or packages.count(package) != 1:
            return "NOT_UNIQUE", "selected stage does not contain package exactly once"
        return "OK", "one W0 stage row containing package"

    match = re.fullmatch(
        r"(authorized_packages|conditional_packages)\["
        r"([a-z][a-z0-9-]*)\]",
        expression,
    )
    if match:
        if path != "swarm/launch-state.toml" or match.group(2) != package:
            return "UNSUPPORTED", "launch selector path or package mismatch"
        values = document.get(match.group(1))
        if isinstance(values, list) and values.count(package) == 1:
            return "OK", "one launch membership"
        return "NOT_UNIQUE", "launch membership did not resolve exactly once"

    match = re.fullmatch(
        r"conditional_activation\.([a-z][a-z0-9-]*)", expression
    )
    if match:
        if path != "swarm/launch-state.toml" or match.group(1) != package:
            return "UNSUPPORTED", "conditional activation path or package mismatch"
        table = document.get("conditional_activation")
        if isinstance(table, dict) and isinstance(table.get(package), dict):
            return "OK", "one conditional activation table"
        return "NOT_UNIQUE", "conditional activation did not resolve exactly once"

    return "UNSUPPORTED", "unsupported selector expression"


def validate_context(
    view: GitView, pair: DraftPair, package: str, checks: Checks
) -> list[dict[str, Any]]:
    result: list[dict[str, Any]] = []
    total_bytes = 0
    for index, path in enumerate(pair.sources):
        check_id = f"source-{index:02d}"
        forbidden = (
            not safe_path(path)
            or path.startswith("docs/architecture/")
            or path.startswith("bins/")
            or re.match(r"^crates/.+/src/", path) is not None
            or any(under(path, root) for root in CONTROL_ROOTS)
        )
        if forbidden:
            checks.fail(
                check_id,
                "CONTEXT_SOURCE_FORBIDDEN",
                f"forbidden context source: {path}",
            )
            continue
        try:
            raw, entry = view.read_bytes(path)
            raw.decode("utf-8", "strict")
        except PlannerFailure as exc:
            reason = (
                exc.reason_code
                if exc.reason_code
                in {
                    "CONTEXT_SOURCE_MISSING",
                    "CONTEXT_SOURCE_NOT_REGULAR",
                    "CONTEXT_BUDGET_EXCEEDED",
                }
                else "CONTEXT_SOURCE_MISSING"
            )
            checks.fail(check_id, reason, exc.message)
            continue
        except UnicodeDecodeError:
            checks.fail(
                check_id,
                "CONTEXT_SOURCE_NOT_UTF8",
                f"context source is not UTF-8: {path}",
            )
            continue
        total_bytes += len(raw)
        checks.pass_(check_id, f"exact regular UTF-8 Git blob: {path}")
        result.append(
            {
                "order": index,
                "path": path,
                "git_blob_id": view.blob_identity(entry),
                "exact_sha256": exact_sha256(raw),
                "exact_bytes": len(raw),
            }
        )
    if total_bytes <= 16 * 1024 * 1024:
        checks.pass_(
            "context-total-bytes",
            f"declared context source bytes are bounded: {total_bytes}",
        )
    else:
        checks.fail(
            "context-total-bytes",
            "CONTEXT_BUDGET_EXCEEDED",
            "declared context exceeds 16 MiB planner ceiling",
        )

    for index, selector in enumerate(pair.selectors):
        status, detail = resolve_selector(view, selector, package)
        if status == "OK":
            checks.pass_(
                f"selector-{index:02d}",
                f"selector resolved exactly once: {selector}",
            )
        else:
            reason = (
                "CONTEXT_SELECTOR_INVALID"
                if status == "UNSUPPORTED"
                else "CONTEXT_SELECTOR_NOT_UNIQUE"
            )
            checks.fail(
                f"selector-{index:02d}",
                reason,
                f"{detail}: {selector}",
            )
    return result


def validate_registries(
    view: GitView, package: str, checks: Checks
) -> tuple[
    dict[str, Any],
    Mapping[str, Any] | None,
    Mapping[str, Any] | None,
    Mapping[str, Any] | None,
]:
    try:
        launch, _ = view.load_toml("swarm/launch-state.toml")
        crates, _ = view.load_toml("swarm/crates.toml")
        functions, _ = view.load_toml("swarm/function-packets.toml")
        stages, _ = view.load_toml("swarm/stages.toml")
    except PlannerFailure as exc:
        checks.fail("registry-files", "CONTROL_SCHEMA_MISMATCH", exc.message)
        return {}, None, None, None

    package_row = one_table(crates.get("package"), "name", package)
    function_row = one_table(functions.get("foundation"), "package", package)
    stage_row = one_table(stages.get("stage"), "id", "W0")

    if package_row is None:
        checks.fail(
            "package-registry",
            "PACKAGE_UNKNOWN",
            "package entry is missing or duplicate",
        )
    else:
        checks.pass_("package-registry", "unique package registry entry")
    if function_row is None:
        checks.fail(
            "function-registry",
            "PACKAGE_REGISTRY_MISMATCH",
            "P00 foundation function entry is missing or duplicate",
        )
    else:
        checks.pass_(
            "function-registry", "unique P00 foundation function entry"
        )
    if (
        stage_row is None
        or not isinstance(stage_row.get("packages"), list)
        or stage_row["packages"].count(package) != 1
    ):
        checks.fail(
            "stage-registry",
            "PACKAGE_STAGE_MISMATCH",
            "package is not exactly once in W0",
        )
    else:
        checks.pass_("stage-registry", "package belongs exactly once to W0")

    if package_row and function_row:
        path = package_row.get("path")
        scope = f"{path}/**" if isinstance(path, str) else ""
        coherent = (
            isinstance(path, str)
            and safe_path(path)
            and package_row.get("family") == "foundation"
            and package_row.get("wave") == 0
            and function_row.get("wave") == 0
            and function_row.get("assignment") == package_row.get("assignment")
            and function_row.get("write_scope") == scope
        )
        if coherent:
            checks.pass_(
                "package-scope", f"package-only write scope: {scope}"
            )
        else:
            checks.fail(
                "package-scope",
                "PACKAGE_REGISTRY_MISMATCH",
                "path/family/wave/assignment/write-scope mismatch",
            )

    launch_ok = (
        launch.get("schema_version") == 6
        and launch.get("active_stage") == "P00"
        and launch.get("active_wave") == 0
    )
    if launch_ok:
        checks.pass_("launch-stage", "launch state remains P00/W0")
    else:
        checks.fail(
            "launch-stage",
            "PACKAGE_STAGE_MISMATCH",
            "launch state is not schema-v6 P00/W0",
        )
    return launch, package_row, function_row, stage_row


def validate_control_schema(
    view: GitView, launch: Mapping[str, Any], checks: Checks
) -> None:
    required = {
        "swarm/orchestration.toml": (5, None),
        "swarm/control-plane-schema.toml": (3, None),
        "swarm/schemas/types-v1.toml": (2, None),
        "swarm/ticket-issuance-plan-schema-v2.toml": (2, RECORD_KIND),
        "swarm/ticket-issuance-plan-digest-v2.toml": (2, None),
        "swarm/ticket-issuance-planner-v2.toml": (2, None),
        "swarm/p00-foundation-acceptance.toml": (1, None),
    }
    loaded: dict[str, dict[str, Any]] = {}
    try:
        for path, _expected in required.items():
            loaded[path], _ = view.load_toml(path)
    except PlannerFailure as exc:
        checks.fail("control-schema", "CONTROL_SCHEMA_MISMATCH", exc.message)
        return

    coherent = all(
        loaded[path].get("schema_version") == expected[0]
        for path, expected in required.items()
    )
    coherent = (
        coherent
        and loaded["swarm/ticket-issuance-plan-schema-v2.toml"].get(
            "record_kind"
        )
        == RECORD_KIND
        and loaded["swarm/ticket-issuance-plan-digest-v2.toml"].get(
            "self_referential_digest_allowed"
        )
        is False
        and loaded["swarm/ticket-issuance-planner-v2.toml"].get("component")
        == "ticket_issuance_planner_v2"
        and loaded["swarm/orchestration.toml"].get("workflow_policy")
        == "manual_only"
        and loaded["swarm/orchestration.toml"].get(
            "consumer_uses_branch_head"
        )
        is False
        and loaded["swarm/orchestration.toml"].get(
            "consumer_requires_exact_commit_and_api_digest"
        )
        is True
        and launch.get("orchestration_registry_schema_version") == 5
        and launch.get("orchestration_registry_path")
        == "swarm/orchestration.toml"
    )
    if coherent:
        checks.pass_(
            "control-schema",
            "planner, control, orchestration and acceptance schemas agree",
        )
    else:
        checks.fail(
            "control-schema",
            "CONTROL_SCHEMA_MISMATCH",
            "planner/control/orchestration schema or path mismatch",
        )
