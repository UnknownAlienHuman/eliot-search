from __future__ import annotations

import hashlib
import json
import os
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any, Mapping, Sequence

from ticket_issuance_planner_v2.core import (
    REPOSITORY_NAME,
    PlannerFailure,
    canonical_json_bytes,
    exact_sha256,
    safe_path,
    under,
)

SCHEMA_VERSION = 1
RECORD_KIND = "context_artifact_candidate_v1"
STATUS = "ARTIFACT_CANDIDATE_NOT_STORED_NOT_SIGNED"
ARTIFACT_FORMAT = "ELIOT_SWARM_CONTEXT_1"
ARTIFACT_ROOT = "artifacts/context-artifact-candidates"
BUNDLE_MAGIC = b"ELIOT_SWARM_CONTEXT_1\n"
BUNDLE_END = b"--- end-context-artifact ---\n"
CANDIDATE_ID_DOMAIN = b"eliot-search/context-artifact-candidate/v1\0"
CANDIDATE_METADATA_DOMAIN = b"eliot-search/context-artifact-candidate-metadata/v1\0"
MAX_BUNDLE_BYTES = 20 * 1024 * 1024

ADDITIONAL_FAILURE_CODES = (
    "CONTEXT_ARTIFACT_BUILDER_CONTRACT_MISMATCH",
    "CONTEXT_SOURCE_CONTAINS_NUL",
    "REGISTRY_FRAGMENT_NONCANONICAL",
    "BUNDLE_FORMAT_INVALID",
    "BUNDLE_SIZE_EXCEEDED",
    "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT",
    "OUTPUT_PATH_SYMLINK",
    "CANDIDATE_OUTPUT_CONFLICT",
    "CANDIDATE_OUTPUT_WRITE_FAILED",
)

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

UNRESOLVED_MANIFEST_FIELDS = (
    "identity.context_id",
    "identity.operation_id",
    "artifact.ref",
    "verification.readback_verified",
    "signature.created_at",
    "signature.materializer_identity",
    "signature.reviewer_identity",
    "signature.record_sha256",
    "signature.materializer_signature_ref",
    "signature.reviewer_signature_ref",
    "record_path.context_record_sha256",
    "record_path.git_commit",
    "record_path.git_blob_id",
    "record_path.exact_record_file_sha256",
)


class CandidateFailure(Exception):
    def __init__(
        self,
        reason_code: str,
        message: str,
        checks: Sequence[Mapping[str, Any]] | None = None,
    ) -> None:
        super().__init__(message)
        self.reason_code = reason_code
        self.message = message
        self.checks = tuple(checks or ())


@dataclass(frozen=True)
class BundleBlock:
    kind: str
    header: str
    metadata: Mapping[str, Any]
    content: bytes


@dataclass(frozen=True)
class CandidateBuild:
    candidate: Mapping[str, Any]
    candidate_bytes: bytes
    bundle_bytes: bytes
    candidate_relative_path: str
    bundle_relative_path: str


def normalize_utf8_lf(raw: bytes, label: str) -> bytes:
    try:
        text = raw.decode("utf-8", "strict")
    except UnicodeDecodeError as exc:
        raise CandidateFailure(
            "CONTEXT_SOURCE_NOT_UTF8", f"source is not strict UTF-8: {label}"
        ) from exc
    if "\x00" in text:
        raise CandidateFailure(
            "CONTEXT_SOURCE_CONTAINS_NUL", f"source contains NUL: {label}"
        )
    return text.replace("\r\n", "\n").replace("\r", "\n").encode("utf-8")


def require_json_value(value: Any, label: str = "value") -> None:
    if value is None or isinstance(value, float):
        raise CandidateFailure(
            "REGISTRY_FRAGMENT_NONCANONICAL",
            f"{label} contains a forbidden null or floating-point value",
        )
    if isinstance(value, (str, bool, int)):
        return
    if isinstance(value, list):
        for index, item in enumerate(value):
            require_json_value(item, f"{label}[{index}]")
        return
    if isinstance(value, dict):
        for key, item in value.items():
            if not isinstance(key, str):
                raise CandidateFailure(
                    "REGISTRY_FRAGMENT_NONCANONICAL",
                    f"{label} contains a non-string map key",
                )
            require_json_value(item, f"{label}.{key}")
        return
    raise CandidateFailure(
        "REGISTRY_FRAGMENT_NONCANONICAL",
        f"{label} contains unsupported type {type(value).__name__}",
    )


def candidate_id(bundle_bytes: bytes) -> str:
    return hashlib.sha256(CANDIDATE_ID_DOMAIN + bundle_bytes).hexdigest()


def candidate_metadata_digest(candidate_without_digest: Mapping[str, Any]) -> str:
    return hashlib.sha256(
        CANDIDATE_METADATA_DOMAIN + canonical_json_bytes(candidate_without_digest)
    ).hexdigest()


def validate_output_root(root: Path, relative: str) -> Path:
    normalized = relative.replace("\\", "/").rstrip("/")
    if (
        not safe_path(normalized)
        or not under(normalized, ARTIFACT_ROOT)
        or normalized == ARTIFACT_ROOT + "/."
    ):
        raise CandidateFailure(
            "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT",
            f"output root must be {ARTIFACT_ROOT} or a descendant",
        )
    target = root / PurePosixPath(normalized)
    cursor = target
    while cursor != root and cursor != cursor.parent:
        if cursor.exists() and cursor.is_symlink():
            raise CandidateFailure(
                "OUTPUT_PATH_SYMLINK",
                f"output path component is a symlink: {cursor}",
            )
        cursor = cursor.parent
    return target


def _ensure_parent_without_symlink(root: Path, parent: Path) -> None:
    relative_parts = parent.relative_to(root).parts
    cursor = root
    for part in relative_parts:
        cursor = cursor / part
        if cursor.exists():
            if cursor.is_symlink() or not cursor.is_dir():
                raise CandidateFailure(
                    "OUTPUT_PATH_SYMLINK",
                    f"output parent is not a regular directory: {cursor}",
                )
        else:
            cursor.mkdir()


def write_exact_idempotent(root: Path, target: Path, data: bytes) -> None:
    try:
        _ensure_parent_without_symlink(root, target.parent)
        if target.exists():
            if target.is_symlink() or not target.is_file():
                raise CandidateFailure(
                    "OUTPUT_PATH_SYMLINK",
                    f"output target is not a regular file: {target}",
                )
            if target.read_bytes() == data:
                return
            raise CandidateFailure(
                "CANDIDATE_OUTPUT_CONFLICT",
                f"existing candidate output differs: {target}",
            )
        temporary = target.with_name(f".{target.name}.tmp-{os.getpid()}")
        if temporary.exists():
            temporary.unlink()
        temporary.write_bytes(data)
        os.replace(temporary, target)
        if target.read_bytes() != data:
            raise CandidateFailure(
                "CANDIDATE_OUTPUT_WRITE_FAILED",
                f"candidate output readback differs: {target}",
            )
    except CandidateFailure:
        raise
    except OSError as exc:
        raise CandidateFailure(
            "CANDIDATE_OUTPUT_WRITE_FAILED",
            f"unable to write candidate output: {target}: {exc}",
        ) from exc


def planner_failure_from_checks(checks: Sequence[Mapping[str, Any]]) -> CandidateFailure:
    failed = [item for item in checks if item.get("status") == "FAIL"]
    if not failed:
        return CandidateFailure(
            "CONTEXT_ARTIFACT_BUILDER_CONTRACT_MISMATCH",
            "planner preflight failed without a typed failed check",
            checks,
        )
    first = failed[0]
    code = str(first.get("reason_code", "CONTEXT_ARTIFACT_BUILDER_CONTRACT_MISMATCH"))
    return CandidateFailure(code, str(first.get("detail", "planner preflight failed")), checks)


def authority_map() -> dict[str, bool]:
    return {field: False for field in AUTHORITY_FIELDS}


def assert_candidate_digest(candidate: Mapping[str, Any]) -> bool:
    digest = candidate.get("candidate_sha256")
    if not isinstance(digest, str):
        return False
    payload = dict(candidate)
    payload.pop("candidate_sha256", None)
    return digest == candidate_metadata_digest(payload)


def assert_repository_name(value: str) -> None:
    if value != REPOSITORY_NAME:
        raise CandidateFailure(
            "CONTEXT_ARTIFACT_BUILDER_CONTRACT_MISMATCH",
            f"repository identity differs from {REPOSITORY_NAME}",
        )


def map_planner_exception(exc: PlannerFailure) -> CandidateFailure:
    return CandidateFailure(exc.reason_code, exc.message)
