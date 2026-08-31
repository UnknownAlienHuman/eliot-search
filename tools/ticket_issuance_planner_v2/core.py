#!/usr/bin/env python3
"""Read-only schema-v2 P00 ticket-issuance advisory planner.

Every repository input is loaded from one immutable Git commit. The planner can
write only a local JSON artifact below artifacts/ticket-issuance-plans/ or
stdout. It has no control-record mutation path.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any, Iterable, Mapping, Sequence

SCHEMA_VERSION = 2
RECORD_KIND = "ticket_issuance_plan_v2"
STATUS = "ADVISORY_NON_AUTHORITATIVE"
DOMAIN_SEPARATOR = b"eliot-search/ticket-issuance-plan/v2\0"
PLAN_ARTIFACT_ROOT = "artifacts/ticket-issuance-plans"
REPOSITORY_NAME = "UnknownAlienHuman/eliot-search"

DECISION_READY = "READY_FOR_CONTEXT_MATERIALIZATION_PREVIEW"
DECISION_MISSING = "BLOCKED_MISSING_SELECTION"
DECISION_PREREQUISITE = "BLOCKED_PREREQUISITE"
DECISION_CONFLICT = "BLOCKED_CONFLICT"
DECISION_INVALID = "INVALID_REPOSITORY_STATE"

CONTROL_ROOTS = (
    "swarm/context-manifests",
    "swarm/tickets",
    "swarm/leases",
    "swarm/submissions",
    "swarm/reviews",
    "swarm/handoffs",
    "swarm/supersessions",
    "swarm/wave-receipts",
)
CURRENT_PACKAGE_RECORD_ROOTS = (
    "swarm/context-manifests",
    "swarm/tickets",
    "swarm/leases",
    "swarm/submissions",
    "swarm/reviews",
    "swarm/handoffs",
)
ROOT_METADATA_NAMES = ("README.md", ".gitkeep")

CLOSED_REASON_CODES = (
    "GIT_REPOSITORY_INVALID",
    "PACKAGE_UNKNOWN",
    "PACKAGE_STAGE_MISMATCH",
    "PACKAGE_REGISTRY_MISMATCH",
    "DRAFT_MANIFEST_MISMATCH",
    "DRAFT_PAIR_MISSING",
    "DRAFT_PAIR_MISMATCH",
    "DRAFT_UNKNOWN_FIELD",
    "DRAFT_BECAME_CLAIMABLE",
    "DRAFT_IDENTITY_PREMATURELY_RESOLVED",
    "CONTEXT_BUDGET_EXCEEDED",
    "CONTEXT_SOURCE_MISSING",
    "CONTEXT_SOURCE_NOT_REGULAR",
    "CONTEXT_SOURCE_NOT_UTF8",
    "CONTEXT_SOURCE_FORBIDDEN",
    "CONTEXT_SELECTOR_INVALID",
    "CONTEXT_SELECTOR_NOT_UNIQUE",
    "HANDOFF_SLOT_UNSATISFIED",
    "HANDOFF_SET_UNEXPECTED",
    "HANDOFF_RECORD_INVALID",
    "HANDOFF_RECORD_SUPERSEDED",
    "PARTIAL_ISSUANCE_SELECTION",
    "BASE_COMMIT_INVALID",
    "ACTOR_IDENTITY_INVALID",
    "WRITER_REVIEWER_CONFLICT",
    "CURRENT_PACKAGE_CONTROL_RECORD_EXISTS",
    "W0_ALREADY_ACCEPTED",
    "CONTROL_SCHEMA_MISMATCH",
    "WORKFLOW_POLICY_VIOLATION",
    "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT",
    "OUTPUT_PATH_SYMLINK",
    "OUTPUT_WRITE_FAILED",
)

INVALID_REASONS = {
    "GIT_REPOSITORY_INVALID",
    "PACKAGE_UNKNOWN",
    "PACKAGE_STAGE_MISMATCH",
    "PACKAGE_REGISTRY_MISMATCH",
    "DRAFT_MANIFEST_MISMATCH",
    "DRAFT_PAIR_MISSING",
    "DRAFT_PAIR_MISMATCH",
    "DRAFT_UNKNOWN_FIELD",
    "DRAFT_BECAME_CLAIMABLE",
    "DRAFT_IDENTITY_PREMATURELY_RESOLVED",
    "CONTEXT_BUDGET_EXCEEDED",
    "CONTEXT_SOURCE_MISSING",
    "CONTEXT_SOURCE_NOT_REGULAR",
    "CONTEXT_SOURCE_NOT_UTF8",
    "CONTEXT_SOURCE_FORBIDDEN",
    "CONTEXT_SELECTOR_INVALID",
    "CONTEXT_SELECTOR_NOT_UNIQUE",
    "HANDOFF_RECORD_INVALID",
    "HANDOFF_RECORD_SUPERSEDED",
    "BASE_COMMIT_INVALID",
    "ACTOR_IDENTITY_INVALID",
    "CONTROL_SCHEMA_MISMATCH",
    "WORKFLOW_POLICY_VIOLATION",
    "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT",
    "OUTPUT_PATH_SYMLINK",
}
CONFLICT_REASONS = {
    "HANDOFF_SET_UNEXPECTED",
    "PARTIAL_ISSUANCE_SELECTION",
    "WRITER_REVIEWER_CONFLICT",
    "CURRENT_PACKAGE_CONTROL_RECORD_EXISTS",
    "W0_ALREADY_ACCEPTED",
}
PREREQUISITE_REASONS = {"HANDOFF_SLOT_UNSATISFIED"}

SAFE_PATH_RE = re.compile(r"^[A-Za-z0-9._-]+(?:/[A-Za-z0-9._-]+)*$")
PACKAGE_RE = re.compile(r"^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$")
ACTOR_RE = re.compile(
    r"^actor:(?:user|service|reviewer|integration):"
    r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}$"
)
TAGGED_GIT_RE = re.compile(r"^(sha1:[0-9a-f]{40}|sha256:[0-9a-f]{64})$")
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
OPAQUE_ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")

FORBIDDEN_WORKFLOW_TRIGGER_RE = re.compile(
    r"^\s{0,6}(?:push|pull_request|pull_request_target|merge_group|schedule|"
    r"workflow_run|repository_dispatch|workflow_call|release|issues|"
    r"issue_comment|discussion|discussion_comment|create|delete|"
    r"branch_protection_rule|check_run|check_suite|deployment|"
    r"deployment_status|fork|gollum|label|milestone|page_build|project|"
    r"project_card|project_column|public|registry_package|status|watch):\s*$",
    re.MULTILINE,
)

TICKET_ALLOWED = {
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
TICKET_SECTIONS = {
    "unresolved_identity": {
        "ticket_id",
        "writer",
        "reviewer",
        "issued_at",
        "base_commit",
        "branch_or_worktree",
        "ticket_signed_payload_sha256",
        "ticket_exact_record_file_sha256",
        "integration_signature_ref",
    },
    "repository_fence": {
        "repository",
        "write_scope",
        "feature_profile",
        "package_registry_path",
        "function_registry_path",
        "stage_registry_path",
        "launch_state_path",
        "registry_digests",
    },
    "context": {
        "context_draft",
        "context_manifest_ref",
        "context_artifact_ref",
        "context_artifact_sha256",
        "writer_visible_artifact_count",
        "architecture_access",
    },
    "dependencies": {
        "required_handoff_packages",
        "accepted_handoff_refs",
        "required_contract_commit",
        "required_contract_api_schema_digest",
        "status",
    },
    "limits": {
        "soft_src_lines",
        "split_review_total_lines",
        "hard_total_lines",
        "one_active_writer",
    },
    "deliverables": {
        "required_outputs",
        "required_evidence",
        "issuance_requirements",
    },
}
CONTEXT_ALLOWED = {
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
    "materialized_context_manifest_ref",
    "materialized_context_record_sha256",
    "materialized_context_artifact_ref",
    "materialized_context_artifact_sha256",
    "materialization_mode",
    "writer_visible_artifact_count",
    "source_file_count",
    "registry_fragment_count",
    "accepted_handoff_slot_count",
    "canonicalization",
    "content",
}
CONTEXT_SECTIONS = {
    "canonicalization": {
        "encoding",
        "line_endings",
        "path_header_format",
        "registry_header_format",
        "preserve_declared_order",
        "record_source_sha256",
        "record_fragment_sha256",
    },
    "content": {
        "source_files",
        "registry_fragments",
        "accepted_handoff_slots",
        "forbidden_paths",
        "required_unavailable_checks",
    },
}


class PlannerFailure(Exception):
    def __init__(self, reason_code: str, message: str) -> None:
        super().__init__(message)
        self.reason_code = reason_code
        self.message = message


@dataclass(frozen=True)
class GitEntry:
    path: str
    mode: str
    object_type: str
    object_id: str

    @property
    def regular_blob(self) -> bool:
        return self.object_type == "blob" and self.mode in {"100644", "100755"}


class GitView:
    """Immutable Git-tree reader."""

    def __init__(self, root: Path, tagged_commit: str | None) -> None:
        self.root = root.resolve()
        probe = self._run("rev-parse", "--git-dir")
        if probe.returncode != 0:
            raise PlannerFailure("GIT_REPOSITORY_INVALID", "repository root is not a Git repository")
        self.object_format = self._run_text("rev-parse", "--show-object-format").strip()
        if self.object_format not in {"sha1", "sha256"}:
            raise PlannerFailure("GIT_REPOSITORY_INVALID", "unsupported Git object format")
        if tagged_commit is None:
            oid = self._run_text("rev-parse", "HEAD").strip().lower()
        else:
            if TAGGED_GIT_RE.fullmatch(tagged_commit) is None:
                raise PlannerFailure(
                    "BASE_COMMIT_INVALID",
                    "base commit must be a full algorithm-tagged Git object ID",
                )
            algorithm, oid = tagged_commit.split(":", 1)
            if algorithm != self.object_format:
                raise PlannerFailure(
                    "BASE_COMMIT_INVALID",
                    "base commit algorithm differs from repository object format",
                )
            if self._run("cat-file", "-e", f"{oid}^{{commit}}").returncode != 0:
                raise PlannerFailure(
                    "BASE_COMMIT_INVALID",
                    "base commit does not exist as a commit object",
                )
        expected = 40 if self.object_format == "sha1" else 64
        if re.fullmatch(rf"[0-9a-f]{{{expected}}}", oid) is None:
            raise PlannerFailure(
                "GIT_REPOSITORY_INVALID", "resolved commit object ID is invalid"
            )
        self.oid = oid
        self.tagged_commit = f"{self.object_format}:{oid}"

    def _run(self, *args: str) -> subprocess.CompletedProcess[bytes]:
        return subprocess.run(
            ["git", "-C", str(self.root), *args],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def _run_text(self, *args: str) -> str:
        completed = self._run(*args)
        if completed.returncode != 0:
            raise PlannerFailure(
                "GIT_REPOSITORY_INVALID",
                completed.stderr.decode("utf-8", "replace").strip()
                or "Git command failed",
            )
        return completed.stdout.decode("utf-8", "strict")

    def entry(self, path: str) -> GitEntry | None:
        if not safe_path(path):
            return None
        completed = self._run("ls-tree", "-z", self.oid, "--", path)
        if completed.returncode != 0:
            raise PlannerFailure("GIT_REPOSITORY_INVALID", "git ls-tree failed")
        exact: list[GitEntry] = []
        for record in (item for item in completed.stdout.split(b"\0") if item):
            meta, raw_path = record.split(b"\t", 1)
            mode, object_type, object_id = meta.decode("ascii").split(" ")
            decoded = raw_path.decode("utf-8", "strict")
            if decoded == path:
                exact.append(GitEntry(decoded, mode, object_type, object_id))
        return exact[0] if len(exact) == 1 else None

    def read_bytes(
        self, path: str, max_bytes: int = 4 * 1024 * 1024
    ) -> tuple[bytes, GitEntry]:
        entry = self.entry(path)
        if entry is None:
            raise PlannerFailure(
                "CONTEXT_SOURCE_MISSING", f"missing committed path: {path}"
            )
        if not entry.regular_blob:
            raise PlannerFailure(
                "CONTEXT_SOURCE_NOT_REGULAR",
                f"path is not a regular committed blob: {path}",
            )
        completed = self._run("cat-file", "blob", entry.object_id)
        if completed.returncode != 0:
            raise PlannerFailure(
                "GIT_REPOSITORY_INVALID", f"unable to read Git blob: {path}"
            )
        if len(completed.stdout) > max_bytes:
            raise PlannerFailure(
                "CONTEXT_BUDGET_EXCEEDED",
                f"committed file exceeds planner byte ceiling: {path}",
            )
        return completed.stdout, entry

    def read_text(
        self, path: str, max_bytes: int = 4 * 1024 * 1024
    ) -> tuple[str, GitEntry]:
        raw, entry = self.read_bytes(path, max_bytes=max_bytes)
        try:
            return raw.decode("utf-8", "strict"), entry
        except UnicodeDecodeError as exc:
            raise PlannerFailure(
                "CONTEXT_SOURCE_NOT_UTF8",
                f"committed file is not UTF-8: {path}",
            ) from exc

    def load_toml(self, path: str) -> tuple[dict[str, Any], GitEntry]:
        raw, entry = self.read_bytes(path)
        try:
            value = tomllib.loads(raw.decode("utf-8", "strict"))
        except (UnicodeDecodeError, tomllib.TOMLDecodeError) as exc:
            raise PlannerFailure(
                "CONTROL_SCHEMA_MISMATCH",
                f"invalid committed TOML: {path}: {exc}",
            ) from exc
        if not isinstance(value, dict):
            raise PlannerFailure(
                "CONTROL_SCHEMA_MISMATCH", f"invalid TOML root: {path}"
            )
        return value, entry

    def list_files(self, prefix: str) -> list[str]:
        if not safe_path(prefix):
            raise PlannerFailure(
                "CONTROL_SCHEMA_MISMATCH", f"unsafe tree prefix: {prefix}"
            )
        completed = self._run(
            "ls-tree", "-r", "-z", "--name-only", self.oid, "--", prefix
        )
        if completed.returncode != 0:
            raise PlannerFailure(
                "GIT_REPOSITORY_INVALID", f"unable to list tree: {prefix}"
            )
        return sorted(
            raw.decode("utf-8", "strict")
            for raw in completed.stdout.split(b"\0")
            if raw
        )

    def blob_identity(self, entry: GitEntry) -> str:
        return f"{self.object_format}:{entry.object_id}"

    def commit_exists(self, tagged: str) -> bool:
        if TAGGED_GIT_RE.fullmatch(tagged) is None:
            return False
        algorithm, oid = tagged.split(":", 1)
        return (
            algorithm == self.object_format
            and self._run("cat-file", "-e", f"{oid}^{{commit}}").returncode == 0
        )


def safe_path(value: str) -> bool:
    if not isinstance(value, str) or SAFE_PATH_RE.fullmatch(value) is None:
        return False
    pure = PurePosixPath(value)
    return not pure.is_absolute() and all(
        part not in {"", ".", ".."} for part in pure.parts
    )


def under(path: str, prefix: str) -> bool:
    return path == prefix or path.startswith(prefix + "/")


def exact_sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def canonical_json_bytes(value: Mapping[str, Any]) -> bytes:
    return (
        json.dumps(
            value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
        )
        + "\n"
    ).encode("utf-8")


def plan_digest(payload_without_digest: Mapping[str, Any]) -> str:
    return hashlib.sha256(
        DOMAIN_SEPARATOR + canonical_json_bytes(payload_without_digest)
    ).hexdigest()


def as_string_array(value: Any, label: str) -> tuple[str, ...]:
    if not isinstance(value, list) or any(not isinstance(item, str) for item in value):
        raise PlannerFailure(
            "DRAFT_PAIR_MISMATCH", f"{label} must be an array of strings"
        )
    return tuple(value)


def one_table(
    rows: Any, key: str, expected: str
) -> Mapping[str, Any] | None:
    if not isinstance(rows, list):
        return None
    matches = [
        row
        for row in rows
        if isinstance(row, dict) and row.get(key) == expected
    ]
    return matches[0] if len(matches) == 1 else None


class Checks:
    def __init__(self) -> None:
        self.items: list[dict[str, str]] = []
        self.reasons: list[str] = []

    def pass_(self, check_id: str, detail: str) -> None:
        self.items.append({"id": check_id, "status": "PASS", "detail": detail})

    def fail(self, check_id: str, reason: str, detail: str) -> None:
        if reason not in CLOSED_REASON_CODES:
            raise AssertionError(f"unregistered planner reason: {reason}")
        self.items.append(
            {
                "id": check_id,
                "status": "FAIL",
                "reason_code": reason,
                "detail": detail,
            }
        )
        if reason not in self.reasons:
            self.reasons.append(reason)


@dataclass(frozen=True)
class DraftPair:
    ticket_path: str
    context_path: str
    ticket: Mapping[str, Any]
    context: Mapping[str, Any]
    sources: tuple[str, ...]
    selectors: tuple[str, ...]
    handoff_slots: tuple[str, ...]
    unavailable_checks: tuple[str, ...]
    source_ceiling_class: str
    ticket_blob: str
    ticket_sha256: str
    context_blob: str
    context_sha256: str


def validate_keys(
    checks: Checks,
    check_id: str,
    value: Mapping[str, Any],
    allowed: set[str],
) -> None:
    unknown = sorted(set(value) - allowed)
    if unknown:
        checks.fail(
            check_id,
            "DRAFT_UNKNOWN_FIELD",
            "unknown fields: " + ",".join(unknown),
        )
    else:
        checks.pass_(check_id, "field set is closed")


def expected_contract_pack_sources(
    view: GitView, package: str
) -> tuple[str, ...]:
    manifest, _ = view.load_toml("docs/contracts/p00/manifest.toml")
    required = as_string_array(manifest.get("required_files"), "required_files")
    if not required or required[0] != "README.md" or len(set(required)) != len(required):
        raise PlannerFailure(
            "DRAFT_MANIFEST_MISMATCH",
            "P00 contract manifest required_files is not canonical",
        )
    return (
        "AGENTS.md",
        f"crates/{package}/AGENTS.md",
        "docs/handoff/AUTHORITY_MAP.md",
        "swarm/ASSIGNMENT_PROTOCOL.md",
        f"swarm/assignments/{package}.md",
        "docs/handoff/P00_BOOTSTRAP.md",
        "docs/contracts/p00/README.md",
        "docs/contracts/p00/manifest.toml",
        *tuple(f"docs/contracts/p00/{name}" for name in required[1:]),
    )
