#!/usr/bin/env python3
"""Read-only ELIOT Search ticket-issuance preflight planner.

The planner emits an advisory canonical JSON decision. It deliberately has no code path that writes a
context manifest, assignment ticket, lease, submission, review, handoff, gate receipt, wave receipt or
launch-state change.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any, Iterable, Mapping, Sequence

SCHEMA_VERSION = 1
RECORD_KIND = "ticket_issuance_plan_v1"
STATUS = "ADVISORY_NON_AUTHORITATIVE"
DOMAIN_SEPARATOR = b"eliot-search/ticket-issuance-plan/v1\0"

DECISION_READY = "READY_FOR_CONTEXT_MATERIALIZATION_PREVIEW"
DECISION_MISSING = "BLOCKED_MISSING_SELECTION"
DECISION_PREREQUISITE = "BLOCKED_PREREQUISITE"
DECISION_CONFLICT = "BLOCKED_CONFLICT"
DECISION_INVALID = "INVALID_REPOSITORY_STATE"

PROTECTED_ROOTS = (
    "swarm/context-manifests",
    "swarm/tickets",
    "swarm/leases",
    "swarm/submissions",
    "swarm/reviews",
    "swarm/handoffs",
    "swarm/supersessions",
    "swarm/wave-receipts",
)

CLOSED_REASON_CODES = (
    "PACKAGE_UNKNOWN",
    "PACKAGE_STAGE_MISMATCH",
    "PACKAGE_REGISTRY_MISMATCH",
    "DRAFT_PAIR_MISSING",
    "DRAFT_PAIR_MISMATCH",
    "DRAFT_BECAME_CLAIMABLE",
    "DRAFT_IDENTITY_PREMATURELY_RESOLVED",
    "CONTEXT_SOURCE_MISSING",
    "CONTEXT_SOURCE_SYMLINK",
    "CONTEXT_SOURCE_FORBIDDEN",
    "CONTEXT_SELECTOR_INVALID",
    "HANDOFF_SLOT_UNSATISFIED",
    "HANDOFF_SET_UNEXPECTED",
    "PARTIAL_ISSUANCE_SELECTION",
    "BASE_COMMIT_INVALID",
    "BASE_COMMIT_SOURCE_MISSING",
    "ACTOR_IDENTITY_INVALID",
    "WRITER_REVIEWER_CONFLICT",
    "PROTECTED_ROOT_NOT_ZERO_STATE",
    "ACTIVE_LEASE_CONFLICT",
    "CONTROL_SCHEMA_MISMATCH",
    "WORKFLOW_POLICY_VIOLATION",
    "OUTPUT_PATH_PROTECTED",
    "OUTPUT_WRITE_FAILED",
)

INVALID_REASONS = {
    "PACKAGE_UNKNOWN",
    "PACKAGE_STAGE_MISMATCH",
    "PACKAGE_REGISTRY_MISMATCH",
    "DRAFT_PAIR_MISSING",
    "DRAFT_PAIR_MISMATCH",
    "DRAFT_BECAME_CLAIMABLE",
    "DRAFT_IDENTITY_PREMATURELY_RESOLVED",
    "CONTEXT_SOURCE_MISSING",
    "CONTEXT_SOURCE_SYMLINK",
    "CONTEXT_SOURCE_FORBIDDEN",
    "CONTEXT_SELECTOR_INVALID",
    "BASE_COMMIT_INVALID",
    "BASE_COMMIT_SOURCE_MISSING",
    "ACTOR_IDENTITY_INVALID",
    "CONTROL_SCHEMA_MISMATCH",
    "WORKFLOW_POLICY_VIOLATION",
    "OUTPUT_PATH_PROTECTED",
}
CONFLICT_REASONS = {
    "PARTIAL_ISSUANCE_SELECTION",
    "WRITER_REVIEWER_CONFLICT",
    "PROTECTED_ROOT_NOT_ZERO_STATE",
    "ACTIVE_LEASE_CONFLICT",
    "HANDOFF_SET_UNEXPECTED",
}
PREREQUISITE_REASONS = {"HANDOFF_SLOT_UNSATISFIED"}

SAFE_PATH_RE = re.compile(r"^[A-Za-z0-9._-]+(?:/[A-Za-z0-9._-]+)*$")
ACTOR_RE = re.compile(r"^actor:(?:user|service|reviewer|integration):[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")
TAGGED_GIT_RE = re.compile(r"^(sha1:[0-9a-f]{40}|sha256:[0-9a-f]{64})$")
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")

SELECTOR_PATTERNS = (
    re.compile(r"^swarm/crates\.toml::package\[name=[a-z][a-z0-9-]*\]$"),
    re.compile(r"^swarm/function-packets\.toml::foundation\[package=[a-z][a-z0-9-]*\]$"),
    re.compile(r"^swarm/function-packets\.toml::package\[name=[a-z][a-z0-9-]*\]$"),
    re.compile(r"^swarm/stages\.toml::stage\[id=W(?:10|[0-9])\]$"),
    re.compile(r"^swarm/launch-state\.toml::authorized_packages\[[a-z][a-z0-9-]*\]$"),
    re.compile(r"^swarm/launch-state\.toml::conditional_packages\[[a-z][a-z0-9-]*\]$"),
    re.compile(r"^swarm/launch-state\.toml::conditional_activation\.[a-z][a-z0-9-]*$"),
)

FORBIDDEN_WORKFLOW_TRIGGER_RE = re.compile(
    r"^\s{0,6}(?:push|pull_request|pull_request_target|merge_group|schedule|workflow_run|"
    r"repository_dispatch|workflow_call|release|issues|issue_comment|discussion|"
    r"discussion_comment|create|delete|branch_protection_rule|check_run|check_suite|"
    r"deployment|deployment_status|fork|gollum|label|milestone|page_build|project|"
    r"project_card|project_column|public|registry_package|status|watch):\s*$",
    re.MULTILINE,
)

TICKET_TOP_LEVEL = {
    "schema_version",
    "record_kind",
    "status",
    "claimable",
    "authorizes_implementation",
    "creates_lease",
    "may_be_writer_acknowledged",
    "package",
    "stage",
    "phase",
    "wave",
    "launch_class",
    "launch_precondition",
    "issuance_status",
    "unresolved_identity",
    "repository_fence",
    "context",
    "dependencies",
    "limits",
    "deliverables",
}
CONTEXT_TOP_LEVEL = {
    "schema_version",
    "record_kind",
    "status",
    "claimable",
    "authorizes_implementation",
    "package",
    "stage",
    "phase",
    "wave",
    "base_commit",
    "materialized_context_ref",
    "materialized_context_sha256",
    "materialization_mode",
    "writer_visible_artifact_count",
    "source_file_count",
    "registry_fragment_count",
    "accepted_handoff_slot_count",
    "canonicalization",
    "content",
}


class PlannerFailure(Exception):
    """Fatal planner failure before a canonical plan can be constructed."""

    def __init__(self, reason_code: str, message: str) -> None:
        super().__init__(message)
        self.reason_code = reason_code
        self.message = message


@dataclass(frozen=True)
class AcceptedHandoff:
    package: str
    path: str
    exact_file_sha256: str
    accepted_commit: str
    api_schema_digest: str
    evidence_digest: str


class Checks:
    def __init__(self) -> None:
        self.items: list[dict[str, Any]] = []
        self.reason_codes: list[str] = []

    def pass_(self, check_id: str, detail: str) -> None:
        self.items.append({"id": check_id, "status": "PASS", "detail": detail})

    def fail(self, check_id: str, reason_code: str, detail: str) -> None:
        if reason_code not in CLOSED_REASON_CODES:
            raise AssertionError(f"unregistered reason code: {reason_code}")
        self.items.append(
            {"id": check_id, "status": "FAIL", "reason_code": reason_code, "detail": detail}
        )
        if reason_code not in self.reason_codes:
            self.reason_codes.append(reason_code)


@dataclass(frozen=True)
class DraftPair:
    ticket_path: str
    context_path: str
    ticket: Mapping[str, Any]
    context: Mapping[str, Any]
    sources: tuple[str, ...]
    selectors: tuple[str, ...]
    handoff_slots: tuple[str, ...]


def load_toml(path: Path) -> dict[str, Any]:
    try:
        with path.open("rb") as handle:
            value = tomllib.load(handle)
    except FileNotFoundError as exc:
        raise PlannerFailure("CONTROL_SCHEMA_MISMATCH", f"missing required file: {path.name}") from exc
    except tomllib.TOMLDecodeError as exc:
        raise PlannerFailure("CONTROL_SCHEMA_MISMATCH", f"invalid TOML: {path.name}: {exc}") from exc
    if not isinstance(value, dict):
        raise PlannerFailure("CONTROL_SCHEMA_MISMATCH", f"invalid TOML root: {path.name}")
    return value


def canonical_json_bytes(value: Mapping[str, Any]) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n"
    ).encode("utf-8")


def plan_digest(payload_without_digest: Mapping[str, Any]) -> str:
    return hashlib.sha256(DOMAIN_SEPARATOR + canonical_json_bytes(payload_without_digest)).hexdigest()


def exact_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def safe_repository_path(value: str) -> bool:
    if not SAFE_PATH_RE.fullmatch(value):
        return False
    pure = PurePosixPath(value)
    return not pure.is_absolute() and all(part not in {"", ".", ".."} for part in pure.parts)


def is_under(relative_path: str, root: str) -> bool:
    return relative_path == root or relative_path.startswith(root + "/")


def regular_non_symlink(root: Path, relative_path: str) -> bool:
    path = root / relative_path
    return path.is_file() and not path.is_symlink()


def run_git(root: Path, args: Sequence[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", "-C", str(root), *args],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
        errors="replace",
    )


def current_git_identity(root: Path) -> tuple[str, str]:
    object_format = run_git(root, ["rev-parse", "--show-object-format"])
    head = run_git(root, ["rev-parse", "HEAD"])
    if object_format.returncode != 0 or head.returncode != 0:
        return "UNAVAILABLE", "UNAVAILABLE"
    algorithm = object_format.stdout.strip()
    oid = head.stdout.strip().lower()
    if algorithm not in {"sha1", "sha256"}:
        return "UNAVAILABLE", "UNAVAILABLE"
    return algorithm, f"{algorithm}:{oid}"


def validate_tagged_commit(root: Path, tagged: str) -> tuple[bool, str]:
    if not TAGGED_GIT_RE.fullmatch(tagged):
        return False, "base commit must be a full algorithm-tagged Git object ID"
    algorithm, oid = tagged.split(":", 1)
    object_format = run_git(root, ["rev-parse", "--show-object-format"])
    if object_format.returncode != 0 or object_format.stdout.strip() != algorithm:
        return False, "base commit algorithm does not match repository object format"
    probe = run_git(root, ["cat-file", "-e", f"{oid}^{{commit}}"])
    if probe.returncode != 0:
        return False, "base commit does not exist as a commit object"
    return True, oid


def commit_contains(root: Path, oid: str, relative_path: str) -> bool:
    probe = run_git(root, ["cat-file", "-e", f"{oid}:{relative_path}"])
    return probe.returncode == 0


def array_of_strings(value: Any) -> tuple[str, ...]:
    if not isinstance(value, list) or any(not isinstance(item, str) for item in value):
        raise PlannerFailure("DRAFT_PAIR_MISMATCH", "expected an array of strings")
    return tuple(value)


def find_table(rows: Any, key: str, expected: str) -> Mapping[str, Any] | None:
    if not isinstance(rows, list):
        return None
    matches = [row for row in rows if isinstance(row, dict) and row.get(key) == expected]
    return matches[0] if len(matches) == 1 else None


def manifest_draft_path(manifest: Mapping[str, Any], package: str) -> str | None:
    row = find_table(manifest.get("draft"), "package", package)
    path = row.get("path") if row else None
    return path if isinstance(path, str) else None


def validate_closed_top_level(
    checks: Checks, check_id: str, value: Mapping[str, Any], allowed: set[str]
) -> None:
    unknown = sorted(set(value) - allowed)
    if unknown:
        checks.fail(check_id, "DRAFT_PAIR_MISMATCH", "unknown top-level fields: " + ",".join(unknown))
    else:
        checks.pass_(check_id, "draft top-level field set is closed")


def load_draft_pair(root: Path, package: str, checks: Checks) -> DraftPair | None:
    try:
        ticket_manifest = load_toml(root / "swarm/ticket-drafts/manifest.toml")
        context_manifest = load_toml(root / "swarm/context-drafts/manifest.toml")
    except PlannerFailure as exc:
        checks.fail("draft-manifests", "DRAFT_PAIR_MISSING", exc.message)
        return None

    ticket_path = manifest_draft_path(ticket_manifest, package)
    context_path = manifest_draft_path(context_manifest, package)
    if not ticket_path or not context_path:
        checks.fail("draft-pair-present", "DRAFT_PAIR_MISSING", "ticket/context draft pair is missing")
        return None
    if not safe_repository_path(ticket_path) or not safe_repository_path(context_path):
        checks.fail("draft-pair-paths", "DRAFT_PAIR_MISMATCH", "draft path is not repository-relative safe")
        return None
    if not regular_non_symlink(root, ticket_path) or not regular_non_symlink(root, context_path):
        checks.fail("draft-pair-files", "DRAFT_PAIR_MISSING", "draft file is missing or is a symlink")
        return None

    ticket = load_toml(root / ticket_path)
    context = load_toml(root / context_path)
    validate_closed_top_level(checks, "ticket-draft-fields", ticket, TICKET_TOP_LEVEL)
    validate_closed_top_level(checks, "context-draft-fields", context, CONTEXT_TOP_LEVEL)

    pair_ok = (
        ticket.get("package") == package
        and context.get("package") == package
        and ticket.get("stage") == context.get("stage")
        and ticket.get("phase") == context.get("phase")
        and ticket.get("wave") == context.get("wave")
        and ticket.get("context", {}).get("context_draft") == context_path
    )
    if pair_ok:
        checks.pass_("draft-pair-identity", "ticket and context drafts bind the same package/stage")
    else:
        checks.fail("draft-pair-identity", "DRAFT_PAIR_MISMATCH", "ticket/context identity mismatch")

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
        checks.pass_("draft-nonclaimable", "both drafts remain non-claimable")
    else:
        checks.fail("draft-nonclaimable", "DRAFT_BECAME_CLAIMABLE", "draft authority flags are unsafe")

    unresolved = ticket.get("unresolved_identity")
    unresolved_ok = isinstance(unresolved, dict) and all(
        unresolved.get(key) == value
        for key, value in {
            "ticket_id": "UNASSIGNED",
            "lease_id": "UNASSIGNED",
            "writer": "UNASSIGNED",
            "reviewer": "UNASSIGNED",
            "base_commit": "UNSELECTED",
            "branch_or_worktree": "UNSELECTED",
            "ticket_canonical_digest": "UNAVAILABLE",
        }.items()
    )
    unresolved_ok = unresolved_ok and unresolved.get("issued_at") == "" and unresolved.get(
        "integration_signature_ref"
    ) == ""
    unresolved_ok = unresolved_ok and context.get("base_commit") == "UNSELECTED"
    unresolved_ok = unresolved_ok and context.get("materialized_context_ref") == "UNAVAILABLE"
    unresolved_ok = unresolved_ok and context.get("materialized_context_sha256") == "UNAVAILABLE"
    if unresolved_ok:
        checks.pass_("draft-unresolved-identity", "issuance identities remain unresolved")
    else:
        checks.fail(
            "draft-unresolved-identity",
            "DRAFT_IDENTITY_PREMATURELY_RESOLVED",
            "draft contains premature issuance identity",
        )

    content = context.get("content")
    if not isinstance(content, dict):
        checks.fail("context-content", "DRAFT_PAIR_MISMATCH", "context [content] table is missing")
        return None
    try:
        sources = array_of_strings(content.get("source_files"))
        selectors = array_of_strings(content.get("registry_fragments"))
        handoff_slots = array_of_strings(content.get("accepted_handoff_slots"))
    except PlannerFailure as exc:
        checks.fail("context-content-arrays", exc.reason_code, exc.message)
        return None

    counts_ok = (
        context.get("source_file_count") == len(sources)
        and context.get("registry_fragment_count") == len(selectors)
        and context.get("accepted_handoff_slot_count") == len(handoff_slots)
        and len(sources) <= 32
        and len(selectors) <= 16
        and len(handoff_slots) <= 16
        and context.get("writer_visible_artifact_count") == 1
    )
    if counts_ok:
        checks.pass_("context-counts", "declared context counts and ceilings match")
    else:
        checks.fail("context-counts", "DRAFT_PAIR_MISMATCH", "context counts or ceilings mismatch")

    return DraftPair(
        ticket_path=ticket_path,
        context_path=context_path,
        ticket=ticket,
        context=context,
        sources=sources,
        selectors=selectors,
        handoff_slots=handoff_slots,
    )


def validate_context_sources(root: Path, pair: DraftPair, checks: Checks) -> None:
    seen: set[str] = set()
    for index, relative_path in enumerate(pair.sources):
        check_id = f"context-source-{index:02d}"
        if relative_path in seen or not safe_repository_path(relative_path):
            checks.fail(check_id, "CONTEXT_SOURCE_FORBIDDEN", f"unsafe or duplicate path: {relative_path}")
            continue
        seen.add(relative_path)
        if any(is_under(relative_path, protected) for protected in PROTECTED_ROOTS):
            checks.fail(check_id, "CONTEXT_SOURCE_FORBIDDEN", f"protected control path: {relative_path}")
            continue
        if relative_path.startswith("docs/architecture/") or re.match(
            r"^(?:crates|bins)/.+/src/", relative_path
        ):
            checks.fail(check_id, "CONTEXT_SOURCE_FORBIDDEN", f"forbidden implementation path: {relative_path}")
            continue
        path = root / relative_path
        if path.is_symlink():
            checks.fail(check_id, "CONTEXT_SOURCE_SYMLINK", f"source is a symlink: {relative_path}")
        elif not path.is_file():
            checks.fail(check_id, "CONTEXT_SOURCE_MISSING", f"source is missing: {relative_path}")
        else:
            checks.pass_(check_id, f"regular source: {relative_path}")

    for index, selector in enumerate(pair.selectors):
        check_id = f"context-selector-{index:02d}"
        if any(pattern.fullmatch(selector) for pattern in SELECTOR_PATTERNS):
            registry_path = selector.split("::", 1)[0]
            if regular_non_symlink(root, registry_path):
                checks.pass_(check_id, f"closed selector: {selector}")
            else:
                checks.fail(check_id, "CONTEXT_SOURCE_MISSING", f"registry source missing: {registry_path}")
        else:
            checks.fail(check_id, "CONTEXT_SELECTOR_INVALID", f"unsupported selector: {selector}")


def parse_accepted_handoff(spec: str) -> AcceptedHandoff:
    # PACKAGE=PATH,FILE_SHA256,COMMIT,API_SHA256,EVIDENCE_SHA256
    if "=" not in spec:
        raise PlannerFailure("HANDOFF_SET_UNEXPECTED", "accepted handoff must use PACKAGE=... format")
    package, encoded = spec.split("=", 1)
    parts = encoded.split(",")
    if len(parts) != 5:
        raise PlannerFailure("HANDOFF_SET_UNEXPECTED", "accepted handoff must contain five comma fields")
    path, file_sha, commit, api_sha, evidence_sha = parts
    if not re.fullmatch(r"[a-z][a-z0-9-]*", package):
        raise PlannerFailure("HANDOFF_SET_UNEXPECTED", "accepted handoff package is invalid")
    if not safe_repository_path(path):
        raise PlannerFailure("HANDOFF_SET_UNEXPECTED", "accepted handoff path is unsafe")
    if not SHA256_RE.fullmatch(file_sha) or not SHA256_RE.fullmatch(api_sha) or not SHA256_RE.fullmatch(
        evidence_sha
    ):
        raise PlannerFailure("HANDOFF_SET_UNEXPECTED", "accepted handoff digest is invalid")
    if not TAGGED_GIT_RE.fullmatch(commit):
        raise PlannerFailure("HANDOFF_SET_UNEXPECTED", "accepted handoff commit is invalid")
    return AcceptedHandoff(package, path, file_sha, commit, api_sha, evidence_sha)


def validate_handoffs(
    root: Path, pair: DraftPair, supplied: Sequence[AcceptedHandoff], checks: Checks
) -> list[dict[str, str]]:
    expected_packages = [slot.split("::", 1)[0] for slot in pair.handoff_slots]
    supplied_map: dict[str, AcceptedHandoff] = {}
    duplicate = False
    for handoff in supplied:
        if handoff.package in supplied_map:
            duplicate = True
        supplied_map[handoff.package] = handoff
    if duplicate or sorted(supplied_map) != sorted(expected_packages):
        if set(expected_packages) - set(supplied_map):
            checks.fail(
                "accepted-handoff-set",
                "HANDOFF_SLOT_UNSATISFIED",
                "required accepted handoff slot is missing",
            )
        if set(supplied_map) - set(expected_packages) or duplicate:
            checks.fail(
                "accepted-handoff-extra",
                "HANDOFF_SET_UNEXPECTED",
                "unexpected or duplicate accepted handoff supplied",
            )
    else:
        checks.pass_("accepted-handoff-set", "accepted handoff set exactly matches declared slots")

    result: list[dict[str, str]] = []
    for package in sorted(supplied_map):
        handoff = supplied_map[package]
        path = root / handoff.path
        valid = (
            is_under(handoff.path, f"swarm/handoffs/{package}")
            and path.is_file()
            and not path.is_symlink()
            and exact_sha256(path) == handoff.exact_file_sha256
        )
        if valid:
            try:
                record = load_toml(path)
                valid = (
                    record.get("record_kind") == "package_handoff_v1"
                    and record.get("identity", {}).get("package") == package
                    and record.get("accepted_code", {}).get("final_commit") == handoff.accepted_commit
                    and record.get("public_surface", {}).get("api_schema_digest")
                    == handoff.api_schema_digest
                )
            except PlannerFailure:
                valid = False
        if valid:
            checks.pass_(f"accepted-handoff-{package}", f"immutable handoff verified: {handoff.path}")
            result.append(
                {
                    "package": package,
                    "path": handoff.path,
                    "exact_file_sha256": handoff.exact_file_sha256,
                    "accepted_commit": handoff.accepted_commit,
                    "api_schema_digest": handoff.api_schema_digest,
                    "evidence_digest": handoff.evidence_digest,
                }
            )
        else:
            checks.fail(
                f"accepted-handoff-{package}",
                "HANDOFF_SET_UNEXPECTED",
                f"accepted handoff failed exact readback: {handoff.path}",
            )
    return result


def validate_zero_state(root: Path, package: str, checks: Checks) -> None:
    for protected in PROTECTED_ROOTS:
        path = root / protected
        if not path.is_dir():
            checks.fail(
                f"zero-state-{protected.replace('/', '-')}",
                "CONTROL_SCHEMA_MISMATCH",
                f"protected root missing: {protected}",
            )
            continue
        unexpected = sorted(
            child.relative_to(root).as_posix()
            for child in path.rglob("*")
            if child.is_file() and child.name not in {"README.md", ".gitkeep"}
        )
        if unexpected:
            reason = "ACTIVE_LEASE_CONFLICT" if protected == "swarm/leases" and any(
                is_under(item, f"swarm/leases/{package}") for item in unexpected
            ) else "PROTECTED_ROOT_NOT_ZERO_STATE"
            checks.fail(
                f"zero-state-{protected.replace('/', '-')}",
                reason,
                f"protected root contains issued record: {unexpected[0]}",
            )
        else:
            checks.pass_(
                f"zero-state-{protected.replace('/', '-')}", f"zero-state verified: {protected}"
            )


def validate_control_schemas(root: Path, launch: Mapping[str, Any], checks: Checks) -> None:
    try:
        orchestration = load_toml(root / "swarm/orchestration.toml")
        control = load_toml(root / "swarm/control-plane-schema.toml")
        types = load_toml(root / "swarm/schemas/types-v1.toml")
        plan_schema = load_toml(root / "swarm/ticket-issuance-plan-schema.toml")
        digest_profile = load_toml(root / "swarm/ticket-issuance-plan-digest-v1.toml")
    except PlannerFailure as exc:
        checks.fail("control-schema-files", "CONTROL_SCHEMA_MISMATCH", exc.message)
        return

    coherent = (
        orchestration.get("schema_version") == 5
        and control.get("schema_version") == 3
        and types.get("schema_version") == 2
        and launch.get("orchestration_registry_schema_version") == 5
        and launch.get("orchestration_registry_path") == "swarm/orchestration.toml"
        and orchestration.get("control_plane_schema_registry") == "swarm/control-plane-schema.toml"
        and orchestration.get("control_plane_type_registry") == "swarm/schemas/types-v1.toml"
        and plan_schema.get("schema_version") == 1
        and plan_schema.get("record_kind") == RECORD_KIND
        and digest_profile.get("self_referential_digest_allowed") is False
        and digest_profile.get("canonical_payload")
        == "complete_canonical_plan_object_with_plan_sha256_field_omitted"
    )
    if coherent:
        checks.pass_("control-schema-coherence", "control, orchestration and plan schemas are coherent")
    else:
        checks.fail(
            "control-schema-coherence",
            "CONTROL_SCHEMA_MISMATCH",
            "control/orchestration/plan schema version or path mismatch",
        )


def validate_workflows(root: Path, checks: Checks) -> None:
    workflow_dir = root / ".github/workflows"
    files = sorted([*workflow_dir.glob("*.yml"), *workflow_dir.glob("*.yaml")])
    if not files:
        checks.fail("workflow-policy", "WORKFLOW_POLICY_VIOLATION", "no workflows found")
        return
    violations: list[str] = []
    for path in files:
        text = path.read_text(encoding="utf-8")
        relative = path.relative_to(root).as_posix()
        valid = (
            re.search(r"^\s{2}workflow_dispatch:\s*$", text, re.MULTILINE) is not None
            and FORBIDDEN_WORKFLOW_TRIGGER_RE.search(text) is None
            and re.search(r"^\s{2}contents:\s*read\s*$", text, re.MULTILINE) is not None
            and "persist-credentials: false" in text
        )
        if not valid:
            violations.append(relative)
    if violations:
        checks.fail(
            "workflow-policy",
            "WORKFLOW_POLICY_VIOLATION",
            "workflow policy violation: " + ",".join(violations),
        )
    else:
        checks.pass_("workflow-policy", f"{len(files)} workflows are manual/read-only/credential-free")


def validate_registry_and_launch(
    root: Path, package: str, checks: Checks
) -> tuple[dict[str, Any], Mapping[str, Any] | None, Mapping[str, Any] | None, Mapping[str, Any] | None]:
    try:
        launch = load_toml(root / "swarm/launch-state.toml")
        crates = load_toml(root / "swarm/crates.toml")
        functions = load_toml(root / "swarm/function-packets.toml")
        stages = load_toml(root / "swarm/stages.toml")
    except PlannerFailure as exc:
        checks.fail("registry-files", "CONTROL_SCHEMA_MISMATCH", exc.message)
        return {}, None, None, None

    package_row = find_table(crates.get("package"), "name", package)
    function_row = find_table(functions.get("foundation"), "package", package)
    if function_row is None:
        function_row = find_table(functions.get("package"), "name", package)
    stage_row = find_table(stages.get("stage"), "id", "W0")

    if package_row is None:
        checks.fail("package-registry", "PACKAGE_UNKNOWN", f"package is not uniquely registered: {package}")
    else:
        checks.pass_("package-registry", f"package registry entry found: {package}")
    if function_row is None:
        checks.fail("function-registry", "PACKAGE_REGISTRY_MISMATCH", "function packet entry missing")
    else:
        checks.pass_("function-registry", "function/foundation packet entry found")
    if stage_row is None or package not in stage_row.get("packages", []):
        checks.fail("stage-registry", "PACKAGE_STAGE_MISMATCH", "package is not in W0 stage")
    else:
        checks.pass_("stage-registry", "package belongs to W0")

    if package_row and function_row:
        path = package_row.get("path")
        expected_scope = f"{path}/**" if isinstance(path, str) else None
        coherent = (
            isinstance(path, str)
            and safe_repository_path(path)
            and package_row.get("wave") == 0
            and function_row.get("wave") == 0
            and function_row.get("write_scope") == expected_scope
        )
        if coherent:
            checks.pass_("package-write-scope", f"package-only write scope: {expected_scope}")
        else:
            checks.fail(
                "package-write-scope",
                "PACKAGE_REGISTRY_MISMATCH",
                "package path/wave/write-scope mismatch",
            )

    active = launch.get("active_stage") == "P00" and launch.get("active_wave") == 0
    if active:
        checks.pass_("launch-stage", "launch state remains P00/W0")
    else:
        checks.fail("launch-stage", "PACKAGE_STAGE_MISMATCH", "active launch stage is not P00/W0")

    return launch, package_row, function_row, stage_row


def validate_selection(
    root: Path,
    base_commit: str | None,
    writer: str | None,
    reviewer: str | None,
    pair: DraftPair | None,
    checks: Checks,
) -> tuple[str, str, str, str | None]:
    values = (base_commit, writer, reviewer)
    present = sum(value is not None for value in values)
    if present == 0:
        checks.pass_("issuance-selection", "no issuance selection supplied; advisory missing-selection plan")
        return "NONE", "", "", None
    if present != 3:
        checks.fail(
            "issuance-selection",
            "PARTIAL_ISSUANCE_SELECTION",
            "base commit, writer and reviewer must be selected together",
        )
        return "PARTIAL", base_commit or "", writer or "", reviewer

    assert base_commit is not None and writer is not None and reviewer is not None
    actors_valid = ACTOR_RE.fullmatch(writer) is not None and ACTOR_RE.fullmatch(reviewer) is not None
    if actors_valid:
        checks.pass_("actor-identities", "writer and reviewer use closed ActorIdentity grammar")
    else:
        checks.fail("actor-identities", "ACTOR_IDENTITY_INVALID", "writer or reviewer identity is invalid")
    if writer == reviewer:
        checks.fail("actor-independence", "WRITER_REVIEWER_CONFLICT", "writer and reviewer are identical")
    else:
        checks.pass_("actor-independence", "writer and reviewer identities differ")

    valid_commit, oid_or_message = validate_tagged_commit(root, base_commit)
    if valid_commit:
        checks.pass_("base-commit", "full immutable base commit exists")
        oid = oid_or_message
        if pair:
            all_paths = list(pair.sources) + [selector.split("::", 1)[0] for selector in pair.selectors]
            missing = sorted({path for path in all_paths if not commit_contains(root, oid, path)})
            if missing:
                checks.fail(
                    "base-commit-context",
                    "BASE_COMMIT_SOURCE_MISSING",
                    f"base commit lacks declared source: {missing[0]}",
                )
            else:
                checks.pass_("base-commit-context", "base commit contains every declared context source")
    else:
        checks.fail("base-commit", "BASE_COMMIT_INVALID", oid_or_message)

    return "COMPLETE", base_commit, writer, reviewer


def validate_output_path(root: Path, output: str, checks: Checks) -> Path | None:
    if output == "-":
        checks.pass_("output-path", "plan will be written to stdout")
        return None
    relative = output.replace("\\", "/")
    if not safe_repository_path(relative) or any(is_under(relative, item) for item in PROTECTED_ROOTS):
        checks.fail("output-path", "OUTPUT_PATH_PROTECTED", "output path is unsafe or protected")
        return None
    target = root / relative
    cursor = target.parent
    while cursor != root and cursor != cursor.parent:
        if cursor.is_symlink():
            checks.fail("output-path", "OUTPUT_PATH_PROTECTED", "output parent traverses a symlink")
            return None
        cursor = cursor.parent
    checks.pass_("output-path", f"ordinary advisory output path: {relative}")
    return target


def choose_decision(selection_state: str, reasons: Sequence[str]) -> str:
    reason_set = set(reasons)
    if reason_set & INVALID_REASONS:
        return DECISION_INVALID
    if reason_set & CONFLICT_REASONS:
        return DECISION_CONFLICT
    if reason_set & PREREQUISITE_REASONS:
        return DECISION_PREREQUISITE
    if selection_state == "NONE":
        return DECISION_MISSING
    if selection_state != "COMPLETE":
        return DECISION_CONFLICT
    return DECISION_READY


def build_plan(args: argparse.Namespace) -> tuple[dict[str, Any], Path | None]:
    root = Path(args.root).resolve()
    checks = Checks()
    if not root.is_dir():
        raise PlannerFailure("CONTROL_SCHEMA_MISMATCH", "repository root does not exist")

    launch, package_row, function_row, stage_row = validate_registry_and_launch(root, args.package, checks)
    pair = load_draft_pair(root, args.package, checks)
    if pair:
        validate_context_sources(root, pair, checks)
    validate_control_schemas(root, launch, checks)
    validate_zero_state(root, args.package, checks)
    validate_workflows(root, checks)

    accepted: list[AcceptedHandoff] = []
    for raw in args.accepted_handoff:
        try:
            accepted.append(parse_accepted_handoff(raw))
        except PlannerFailure as exc:
            checks.fail("accepted-handoff-input", exc.reason_code, exc.message)
    accepted_result = validate_handoffs(root, pair, accepted, checks) if pair else []

    classification = "UNKNOWN"
    if args.package in launch.get("authorized_packages", []):
        classification = "AUTHORIZED"
    elif args.package in launch.get("conditional_packages", []):
        classification = "CONDITIONAL"
    expected_class = pair.ticket.get("launch_class") if pair else None
    if classification != "UNKNOWN" and expected_class == classification:
        checks.pass_("launch-classification", f"launch classification: {classification}")
    else:
        checks.fail(
            "launch-classification",
            "PACKAGE_STAGE_MISMATCH",
            "draft and launch classification differ",
        )

    selection_state, selected_base, selected_writer, selected_reviewer = validate_selection(
        root, args.base_commit, args.writer, args.reviewer, pair, checks
    )
    output_target = validate_output_path(root, args.output, checks)

    object_format, checkout_commit = current_git_identity(root)
    package_path = package_row.get("path", "") if package_row else ""
    write_scope = function_row.get("write_scope", "") if function_row else ""
    package_wave = package_row.get("wave", -1) if package_row else -1
    stage_id = pair.ticket.get("stage", "UNKNOWN") if pair else "UNKNOWN"
    phase = pair.ticket.get("phase", "UNKNOWN") if pair else "UNKNOWN"
    required_slots = list(pair.handoff_slots) if pair else []

    decision = choose_decision(selection_state, checks.reason_codes)
    plan: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "record_kind": RECORD_KIND,
        "status": STATUS,
        "repository": {
            "name": "UnknownAlienHuman/eliot-search",
            "object_format": object_format,
            "checkout_commit": checkout_commit,
        },
        "package": {
            "name": args.package,
            "path": package_path,
            "wave": package_wave,
            "write_scope": write_scope,
        },
        "stage": {
            "id": stage_id,
            "phase": phase,
            "active_stage": launch.get("active_stage", "UNKNOWN"),
            "active_wave": launch.get("active_wave", -1),
            "registry_wave": stage_row.get("wave", -1) if stage_row else -1,
        },
        "launch": {
            "classification": classification,
            "eligible_for_preflight": classification in {"AUTHORIZED", "CONDITIONAL"},
            "conditional_requirements": launch.get("conditional_activation", {}).get(args.package, {}),
        },
        "selection": {
            "state": selection_state,
            "base_commit": selected_base,
            "writer": selected_writer,
            "reviewer": selected_reviewer or "",
        },
        "drafts": {
            "ticket_path": pair.ticket_path if pair else "",
            "context_path": pair.context_path if pair else "",
            "source_count": len(pair.sources) if pair else 0,
            "registry_fragment_count": len(pair.selectors) if pair else 0,
            "accepted_handoff_slot_count": len(required_slots),
        },
        "prerequisites": {
            "required_handoff_slots": required_slots,
            "accepted_handoffs": accepted_result,
        },
        "checks": checks.items,
        "decision": decision,
        "reason_codes": checks.reason_codes,
        "mutations": [],
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
        raise PlannerFailure("OUTPUT_WRITE_FAILED", "canonical plan exceeds 262144-byte ceiling")
    if target is None:
        sys.stdout.buffer.write(encoded)
        return
    try:
        target.parent.mkdir(parents=True, exist_ok=True)
        temporary = target.with_name(target.name + ".tmp")
        temporary.write_bytes(encoded)
        temporary.replace(target)
    except OSError as exc:
        raise PlannerFailure("OUTPUT_WRITE_FAILED", f"unable to write advisory output: {exc}") from exc


def parse_args(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".", help="repository root")
    parser.add_argument("--package", required=True, help="exact package name")
    parser.add_argument("--base-commit", help="full algorithm-tagged immutable Git commit")
    parser.add_argument("--writer", help="closed ActorIdentity")
    parser.add_argument("--reviewer", help="closed ActorIdentity distinct from writer")
    parser.add_argument(
        "--accepted-handoff",
        action="append",
        default=[],
        metavar="PACKAGE=PATH,FILE_SHA256,COMMIT,API_SHA256,EVIDENCE_SHA256",
        help="immutable prerequisite package handoff; repeat as needed",
    )
    parser.add_argument(
        "--output",
        default="-",
        help="ordinary repository-relative advisory output path, or '-' for stdout",
    )
    parser.add_argument(
        "--require-ready",
        action="store_true",
        help="return exit 3 unless decision is READY_FOR_CONTEXT_MATERIALIZATION_PREVIEW",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv if argv is not None else sys.argv[1:])
    try:
        plan, target = build_plan(args)
        write_plan(plan, target)
    except PlannerFailure as exc:
        print(f"{exc.reason_code}: {exc.message}", file=sys.stderr)
        return 2
    if args.require_ready and plan["decision"] != DECISION_READY:
        return 3
    return 2 if plan["decision"] == DECISION_INVALID else 0


if __name__ == "__main__":
    raise SystemExit(main())
