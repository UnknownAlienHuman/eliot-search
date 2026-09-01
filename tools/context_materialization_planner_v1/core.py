from __future__ import annotations

import hashlib
import json
import os
import re
from datetime import datetime, timezone
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any, Mapping, Sequence

from context_artifact_builder_v1.core import (
    ARTIFACT_FORMAT,
    CandidateFailure,
    assert_candidate_digest,
    write_exact_idempotent,
)
from ticket_issuance_planner_v2.core import canonical_json_bytes, exact_sha256, safe_path, under

SCHEMA_VERSION = 1
RECORD_KIND = "context_materialization_plan_v1"
STATUS = "ADVISORY_NON_AUTHORITATIVE"
PLAN_ROOT = "artifacts/context-materialization-plans"
PLAN_DOMAIN = b"eliot-search/context-materialization-plan/v1\0"
OPERATION_DOMAIN = b"eliot-search/materialize-context/v1\0"
INSTANCE_STATUS = "MATERIALIZED"
REPOSITORY = "UnknownAlienHuman/eliot-search"

DECISION_MISSING = "BLOCKED_MISSING_EXTERNAL_INPUT"
DECISION_SIGNATURES = "READY_FOR_DUAL_SIGNATURE_COLLECTION"
DECISION_COMMIT = "READY_FOR_INTEGRATION_OWNER_READBACK_AND_COMMIT"
DECISION_PARTIAL_SIGNATURE = "BLOCKED_PARTIAL_SIGNATURE_SET"

SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
OPAQUE_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")
ACTOR_RE = re.compile(r"^actor:(?:user|service|reviewer|integration):[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")
RFC3339_RE = re.compile(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$")

REASON_MISSING_SELECTION = "MATERIALIZATION_SELECTION_MISSING"
REASON_PARTIAL_SIGNATURE = "MATERIALIZATION_SIGNATURE_SET_PARTIAL"

AUTHORITY_FIELDS = (
    "materializes_authoritative_context",
    "creates_context_manifest_record",
    "creates_immutable_artifact_ref",
    "creates_assignment_ticket",
    "creates_writer_lease",
    "authorizes_implementation",
    "publishes_package_handoff",
    "accepts_gate_or_wave",
    "advances_launch_state",
)


class MaterializationPlanError(ValueError):
    def __init__(self, reason_code: str, message: str) -> None:
        super().__init__(message)
        self.reason_code = reason_code
        self.message = message


@dataclass(frozen=True)
class MaterializationBuild:
    plan: Mapping[str, Any]
    plan_bytes: bytes
    payload_bytes: bytes | None
    manifest_bytes: bytes | None
    output_directory: str


def authority_map() -> dict[str, bool]:
    return {field: False for field in AUTHORITY_FIELDS}


def plan_digest(value_without_digest: Mapping[str, Any]) -> str:
    return hashlib.sha256(PLAN_DOMAIN + canonical_json_bytes(value_without_digest)).hexdigest()


def operation_id(input_manifest: Mapping[str, Any]) -> str:
    return hashlib.sha256(OPERATION_DOMAIN + canonical_json_bytes(input_manifest)).hexdigest()


def load_json_file(path: Path, label: str) -> Any:
    try:
        raw = path.read_bytes()
        value = json.loads(raw.decode("utf-8", "strict"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise MaterializationPlanError("MATERIALIZATION_INPUT_INVALID", f"{label}: {exc}") from exc
    if canonical_json_bytes(value) != raw:
        raise MaterializationPlanError(
            "MATERIALIZATION_INPUT_NONCANONICAL", f"{label} is not canonical compact JSON with terminal LF"
        )
    return value


def validate_output_root(root: Path, value: str) -> Path:
    normalized = value.replace("\\", "/").rstrip("/")
    if not safe_path(normalized) or not under(normalized, PLAN_ROOT):
        raise MaterializationPlanError(
            "MATERIALIZATION_OUTPUT_PATH_INVALID",
            f"output root must be {PLAN_ROOT} or a descendant",
        )
    target = root / PurePosixPath(normalized)
    cursor = target
    while cursor != root and cursor != cursor.parent:
        if cursor.exists() and cursor.is_symlink():
            raise MaterializationPlanError(
                "MATERIALIZATION_OUTPUT_PATH_SYMLINK", f"symlink output component: {cursor}"
            )
        cursor = cursor.parent
    return target


def write_artifact(root: Path, relative: str, data: bytes) -> Path:
    target = root / PurePosixPath(relative)
    try:
        write_exact_idempotent(root, target, data)
    except CandidateFailure as exc:
        raise MaterializationPlanError(exc.reason_code, exc.message) from exc
    return target


def require_sha(value: Any, label: str) -> str:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        raise MaterializationPlanError("MATERIALIZATION_INPUT_INVALID", f"{label} is not SHA-256")
    return value


def require_opaque(value: Any, label: str) -> str:
    if not isinstance(value, str) or OPAQUE_RE.fullmatch(value) is None:
        raise MaterializationPlanError("MATERIALIZATION_INPUT_INVALID", f"{label} is not OpaqueId")
    return value


def require_actor(value: Any, label: str) -> str:
    if not isinstance(value, str) or ACTOR_RE.fullmatch(value) is None:
        raise MaterializationPlanError("MATERIALIZATION_INPUT_INVALID", f"{label} is not ActorIdentity")
    return value


def require_rfc3339(value: Any, label: str) -> str:
    if not isinstance(value, str) or RFC3339_RE.fullmatch(value) is None:
        raise MaterializationPlanError("MATERIALIZATION_INPUT_INVALID", f"{label} is not whole-second UTC RFC3339")
    try:
        parsed = datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)
    except ValueError as exc:
        raise MaterializationPlanError("MATERIALIZATION_INPUT_INVALID", f"{label} is not a valid calendar timestamp") from exc
    if parsed.strftime("%Y-%m-%dT%H:%M:%SZ") != value:
        raise MaterializationPlanError("MATERIALIZATION_INPUT_INVALID", f"{label} is not canonical UTC")
    return value


def require_u64(value: Any, label: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0 or value > 2**64 - 1:
        raise MaterializationPlanError("MATERIALIZATION_INPUT_INVALID", f"{label} is not u64")
    return value


def validate_candidate(candidate: Any, bundle_bytes: bytes) -> Mapping[str, Any]:
    if not isinstance(candidate, dict):
        raise MaterializationPlanError("MATERIALIZATION_CANDIDATE_INVALID", "candidate must be an object")
    if (
        candidate.get("schema_version") != 1
        or candidate.get("record_kind") != "context_artifact_candidate_v1"
        or candidate.get("status") != "ARTIFACT_CANDIDATE_NOT_STORED_NOT_SIGNED"
        or candidate.get("reason_codes") != []
        or candidate.get("control_record_mutations") != []
        or not assert_candidate_digest(candidate)
    ):
        raise MaterializationPlanError("MATERIALIZATION_CANDIDATE_INVALID", "candidate identity, status or digest failed")
    authority = candidate.get("authority")
    if not isinstance(authority, dict) or set(authority) != set(AUTHORITY_FIELDS) or any(authority.values()):
        raise MaterializationPlanError("MATERIALIZATION_CANDIDATE_INVALID", "candidate authority ceiling failed")
    artifact = candidate.get("artifact_candidate")
    if not isinstance(artifact, dict):
        raise MaterializationPlanError("MATERIALIZATION_CANDIDATE_INVALID", "artifact_candidate missing")
    if (
        artifact.get("format") != ARTIFACT_FORMAT
        or artifact.get("sha256") != exact_sha256(bundle_bytes)
        or artifact.get("bytes") != len(bundle_bytes)
        or artifact.get("local_file_is_immutable_artifact_ref") is not False
    ):
        raise MaterializationPlanError("MATERIALIZATION_BUNDLE_MISMATCH", "candidate bundle identity mismatch")
    verification = candidate.get("verification")
    projection = candidate.get("manifest_projection")
    if (
        not isinstance(verification, dict)
        or verification.get("bundle_roundtrip_verified") is not True
        or verification.get("authoritative_artifact_store_readback_verified") is not False
        or not isinstance(projection, dict)
        or projection.get("schema_instance") is not False
    ):
        raise MaterializationPlanError("MATERIALIZATION_CANDIDATE_INVALID", "candidate verification/projection failed")
    return candidate


def validate_artifact_ref(value: Any, bundle_bytes: bytes) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != {"store_profile_ref", "artifact_id", "bytes", "sha256"}:
        raise MaterializationPlanError("MATERIALIZATION_ARTIFACT_REF_INVALID", "artifact_ref field set is invalid")
    result = {
        "store_profile_ref": require_opaque(value.get("store_profile_ref"), "artifact_ref.store_profile_ref"),
        "artifact_id": require_opaque(value.get("artifact_id"), "artifact_ref.artifact_id"),
        "bytes": require_u64(value.get("bytes"), "artifact_ref.bytes"),
        "sha256": require_sha(value.get("sha256"), "artifact_ref.sha256"),
    }
    if result["bytes"] != len(bundle_bytes) or result["sha256"] != exact_sha256(bundle_bytes):
        raise MaterializationPlanError("MATERIALIZATION_ARTIFACT_READBACK_MISMATCH", "artifact_ref does not identify bundle bytes")
    return result


def validate_optional_signature(value: Any, expected_actor: str, label: str) -> tuple[str, dict[str, Any] | None]:
    if not isinstance(value, dict) or set(value) != {"state", "value"}:
        raise MaterializationPlanError("MATERIALIZATION_SIGNATURE_REF_INVALID", f"{label} OptionalV1 is invalid")
    state = value.get("state")
    wrapped = value.get("value")
    if state == "ABSENT":
        if wrapped != "":
            raise MaterializationPlanError("MATERIALIZATION_SIGNATURE_REF_INVALID", f"{label} ABSENT requires empty string")
        return state, None
    if state != "PRESENT" or not isinstance(wrapped, dict):
        raise MaterializationPlanError("MATERIALIZATION_SIGNATURE_REF_INVALID", f"{label} state/value is invalid")
    if set(wrapped) != {"approval_profile_ref", "approval_artifact_ref", "signed_payload_sha256", "actor_identity"}:
        raise MaterializationPlanError("MATERIALIZATION_SIGNATURE_REF_INVALID", f"{label} field set is invalid")
    actor = require_actor(wrapped.get("actor_identity"), f"{label}.actor_identity")
    if actor != expected_actor:
        raise MaterializationPlanError("MATERIALIZATION_SIGNATURE_ACTOR_MISMATCH", f"{label} actor differs from selected actor")
    approval_artifact = wrapped.get("approval_artifact_ref")
    if not isinstance(approval_artifact, dict) or set(approval_artifact) != {"store_profile_ref", "artifact_id", "bytes", "sha256"}:
        raise MaterializationPlanError("MATERIALIZATION_SIGNATURE_REF_INVALID", f"{label}.approval_artifact_ref is invalid")
    normalized = {
        "approval_profile_ref": require_opaque(wrapped.get("approval_profile_ref"), f"{label}.approval_profile_ref"),
        "approval_artifact_ref": {
            "store_profile_ref": require_opaque(approval_artifact.get("store_profile_ref"), f"{label}.approval_artifact_ref.store_profile_ref"),
            "artifact_id": require_opaque(approval_artifact.get("artifact_id"), f"{label}.approval_artifact_ref.artifact_id"),
            "bytes": require_u64(approval_artifact.get("bytes"), f"{label}.approval_artifact_ref.bytes"),
            "sha256": require_sha(approval_artifact.get("sha256"), f"{label}.approval_artifact_ref.sha256"),
        },
        "signed_payload_sha256": require_sha(wrapped.get("signed_payload_sha256"), f"{label}.signed_payload_sha256"),
        "actor_identity": actor,
    }
    return state, normalized
