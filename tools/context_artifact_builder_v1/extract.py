from __future__ import annotations

import re
from pathlib import Path
from typing import Any, Mapping, Sequence

from ticket_issuance_planner_v2.context import (
    validate_context,
    validate_control_schema,
    validate_registries,
)
from ticket_issuance_planner_v2.control import (
    validate_handoffs,
    validate_root_metadata_and_package_state,
    validate_workflows,
)
from ticket_issuance_planner_v2.core import (
    Checks,
    DraftPair,
    GitView,
    PlannerFailure,
    canonical_json_bytes,
    exact_sha256,
    one_table,
)
from ticket_issuance_planner_v2.drafts import load_draft_pair

from .core import (
    ARTIFACT_FORMAT,
    ARTIFACT_ROOT,
    RECORD_KIND,
    CandidateFailure,
    BundleBlock,
    map_planner_exception,
    normalize_utf8_lf,
    planner_failure_from_checks,
    require_json_value,
)


def validate_builder_contract(view: GitView) -> None:
    try:
        registry, _ = view.load_toml("swarm/context-artifact-builder-v1.toml")
        schema, _ = view.load_toml("swarm/context-artifact-candidate-schema-v1.toml")
        digest, _ = view.load_toml("swarm/context-artifact-candidate-digest-v1.toml")
    except PlannerFailure as exc:
        raise map_planner_exception(exc) from exc
    authority = registry.get("authority")
    coherent = (
        registry.get("schema_version") == 1
        and registry.get("component") == "context_artifact_builder_v1"
        and registry.get("candidate_schema")
        == "swarm/context-artifact-candidate-schema-v1.toml"
        and registry.get("digest_profile")
        == "swarm/context-artifact-candidate-digest-v1.toml"
        and registry.get("artifact_root") == ARTIFACT_ROOT
        and registry.get("artifact_format") == ARTIFACT_FORMAT
        and schema.get("schema_version") == 1
        and schema.get("record_kind") == RECORD_KIND
        and schema.get("artifact_format") == ARTIFACT_FORMAT
        and digest.get("schema_version") == 1
        and digest.get("profile") == "context_artifact_candidate_digest_v1"
        and digest.get("self_referential_digest_allowed") is False
        and isinstance(authority, dict)
        and authority
        and all(value is False for value in authority.values())
    )
    if not coherent:
        raise CandidateFailure(
            "CONTEXT_ARTIFACT_BUILDER_CONTRACT_MISMATCH",
            "context artifact builder registry/schema/digest contract mismatch",
        )


def preflight(
    root: str,
    package: str,
    base_commit: str,
    accepted_handoff_paths: Sequence[str],
) -> tuple[
    GitView,
    DraftPair,
    Mapping[str, Any],
    Mapping[str, Any],
    list[dict[str, Any]],
    list[dict[str, str]],
]:
    try:
        view = GitView(Path(root), base_commit)
    except PlannerFailure as exc:
        raise map_planner_exception(exc) from exc
    validate_builder_contract(view)

    checks = Checks()
    launch, package_row, _function_row, _stage_row = validate_registries(
        view, package, checks
    )
    pair = load_draft_pair(view, package, package_row, checks)
    if pair is not None:
        validate_context(view, pair, package, checks)
    validate_control_schema(view, launch, checks)
    validate_root_metadata_and_package_state(view, package, checks)
    validate_workflows(view, checks)
    handoffs = (
        validate_handoffs(view, pair, accepted_handoff_paths, checks)
        if pair is not None
        else []
    )
    if pair is None or checks.reasons:
        raise planner_failure_from_checks(checks.items)
    assert package_row is not None
    classification = (
        "AUTHORIZED"
        if package in launch.get("authorized_packages", [])
        else "CONDITIONAL"
        if package in launch.get("conditional_packages", [])
        else "UNKNOWN"
    )
    if pair.ticket.get("launch_class") != classification or classification == "UNKNOWN":
        raise CandidateFailure(
            "PACKAGE_STAGE_MISMATCH",
            "draft and launch classification differ",
            checks.items,
        )
    return view, pair, launch, package_row, handoffs, checks.items


def source_records_and_blocks(
    view: GitView, pair: DraftPair
) -> tuple[list[dict[str, Any]], list[BundleBlock]]:
    canonicalization = pair.context.get("canonicalization")
    if not isinstance(canonicalization, dict):
        raise CandidateFailure("DRAFT_PAIR_MISMATCH", "missing context canonicalization")
    if canonicalization.get("path_header_format") != "--- repository-path: <path> ---":
        raise CandidateFailure(
            "DRAFT_PAIR_MISMATCH", "source header format is not canonical"
        )
    records: list[dict[str, Any]] = []
    blocks: list[BundleBlock] = []
    for order, path in enumerate(pair.sources):
        try:
            raw, entry = view.read_bytes(path)
        except PlannerFailure as exc:
            raise map_planner_exception(exc) from exc
        normalized = normalize_utf8_lf(raw, path)
        record = {
            "order": order,
            "repository_path": path,
            "git_blob_id": view.blob_identity(entry),
            "exact_sha256": exact_sha256(raw),
            "exact_bytes": len(raw),
            "materialization": "UTF8_LF",
            "materialized_sha256": exact_sha256(normalized),
            "materialized_bytes": len(normalized),
        }
        records.append(record)
        blocks.append(
            BundleBlock(
                "source",
                f"--- repository-path: {path} ---",
                record,
                normalized,
            )
        )
    return records, blocks


def _selector_value(
    view: GitView, selector: str, package: str
) -> tuple[str, str, Any, str, str]:
    if "::" not in selector:
        raise CandidateFailure("CONTEXT_SELECTOR_INVALID", f"invalid selector: {selector}")
    path, expression = selector.split("::", 1)
    try:
        document, entry = view.load_toml(path)
        source_raw, _ = view.read_bytes(path)
    except PlannerFailure as exc:
        raise map_planner_exception(exc) from exc

    value: Any = None
    match = re.fullmatch(r"package\[name=([a-z][a-z0-9-]*)\]", expression)
    if match and path == "swarm/crates.toml" and match.group(1) == package:
        value = one_table(document.get("package"), "name", package)
    match = re.fullmatch(r"foundation\[package=([a-z][a-z0-9-]*)\]", expression)
    if match and path == "swarm/function-packets.toml" and match.group(1) == package:
        value = one_table(document.get("foundation"), "package", package)
    match = re.fullmatch(r"stage\[id=(W(?:10|[0-9]))\]", expression)
    if match and path == "swarm/stages.toml" and match.group(1) == "W0":
        value = one_table(document.get("stage"), "id", "W0")
    match = re.fullmatch(
        r"(authorized_packages|conditional_packages)\[([a-z][a-z0-9-]*)\]",
        expression,
    )
    if match and path == "swarm/launch-state.toml" and match.group(2) == package:
        values = document.get(match.group(1))
        if isinstance(values, list) and values.count(package) == 1:
            value = {"membership": match.group(1), "package": package}
        else:
            value = None
    match = re.fullmatch(r"conditional_activation\.([a-z][a-z0-9-]*)", expression)
    if match and path == "swarm/launch-state.toml" and match.group(1) == package:
        table = document.get("conditional_activation")
        value = table.get(package) if isinstance(table, dict) else None

    if value is None:
        raise CandidateFailure(
            "CONTEXT_SELECTOR_NOT_UNIQUE",
            f"selector did not resolve exactly once: {selector}",
        )
    require_json_value(value, selector)
    return (
        path,
        expression,
        value,
        view.blob_identity(entry),
        exact_sha256(source_raw),
    )


def fragment_records_and_blocks(
    view: GitView, pair: DraftPair, package: str
) -> tuple[list[dict[str, Any]], list[BundleBlock]]:
    canonicalization = pair.context.get("canonicalization")
    if not isinstance(canonicalization, dict):
        raise CandidateFailure("DRAFT_PAIR_MISMATCH", "missing context canonicalization")
    if (
        canonicalization.get("registry_header_format")
        != "--- registry-selector: <path>::<selector> ---"
    ):
        raise CandidateFailure(
            "DRAFT_PAIR_MISMATCH", "registry header format is not canonical"
        )
    records: list[dict[str, Any]] = []
    blocks: list[BundleBlock] = []
    for order, selector in enumerate(pair.selectors):
        path, expression, value, blob_id, source_sha = _selector_value(
            view, selector, package
        )
        fragment = canonical_json_bytes(
            {"registry_path": path, "selector": expression, "value": value}
        )
        record = {
            "order": order,
            "registry_path": path,
            "selector": expression,
            "source_git_blob_id": blob_id,
            "source_exact_sha256": source_sha,
            "selector_match_count": 1,
            "fragment_sha256": exact_sha256(fragment),
            "fragment_bytes": len(fragment),
        }
        records.append(record)
        blocks.append(
            BundleBlock(
                "registry_fragment",
                f"--- registry-selector: {path}::{expression} ---",
                record,
                fragment,
            )
        )
    return records, blocks


def handoff_records_and_blocks(
    view: GitView, handoffs: Sequence[Mapping[str, Any]]
) -> tuple[list[dict[str, Any]], list[BundleBlock]]:
    records: list[dict[str, Any]] = []
    blocks: list[BundleBlock] = []
    for order, summary in enumerate(handoffs):
        path = str(summary["path"])
        try:
            raw, _entry = view.read_bytes(path)
            text = raw.decode("utf-8", "strict")
        except (PlannerFailure, UnicodeDecodeError) as exc:
            if isinstance(exc, PlannerFailure):
                raise map_planner_exception(exc) from exc
            raise CandidateFailure(
                "HANDOFF_RECORD_INVALID", f"handoff is not UTF-8: {path}"
            ) from exc
        if "\r" in text or not raw.endswith(b"\n"):
            raise CandidateFailure(
                "HANDOFF_RECORD_INVALID",
                f"handoff is not exact UTF-8/LF with terminal LF: {path}",
            )
        record = dict(summary)
        record.update(
            {
                "order": order,
                "materialization": "EXACT_UTF8_LF",
                "materialized_sha256": exact_sha256(raw),
                "materialized_bytes": len(raw),
            }
        )
        records.append(record)
        blocks.append(
            BundleBlock(
                "accepted_handoff",
                f"--- accepted-handoff: {summary['package']} ---",
                record,
                raw,
            )
        )
    return records, blocks
