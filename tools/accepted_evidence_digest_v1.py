from __future__ import annotations

import hashlib
import json
import re
from typing import Any, Mapping, Sequence

SCHEMA_VERSION = 1
PROFILE = "accepted_evidence_digest_v1"
MAGIC = b"ELIOT_ACCEPTED_EVIDENCE_MANIFEST_1\n"
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
OPAQUE_ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")
CLOSED_ENUM_RE = re.compile(r"^[A-Z][A-Z0-9_]{0,127}$")
MAX_EVIDENCE = 256
MAX_MANIFEST_BYTES = 4 * 1024 * 1024

EXPECTED_FIELDS = (
    "requirement_id",
    "evidence_class",
    "artifact_ref",
    "artifact_sha256",
    "raw_outcome_digest",
    "availability",
)
ARTIFACT_FIELDS = ("store_profile_ref", "artifact_id", "bytes", "sha256")


class EvidenceDigestError(ValueError):
    pass


def canonical_json_bytes(value: Mapping[str, Any]) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        + "\n"
    ).encode("utf-8")


def _is_u64(value: Any) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and 0 <= value <= 2**64 - 1


def _require_exact_keys(value: Mapping[str, Any], expected: Sequence[str], label: str) -> None:
    actual = tuple(value.keys())
    if set(actual) != set(expected) or len(actual) != len(expected):
        raise EvidenceDigestError(f"{label} field set differs from {list(expected)!r}")


def validate_artifact_ref(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise EvidenceDigestError(f"{label} must be an object")
    _require_exact_keys(value, ARTIFACT_FIELDS, label)
    store = value.get("store_profile_ref")
    artifact_id = value.get("artifact_id")
    size = value.get("bytes")
    digest = value.get("sha256")
    if not isinstance(store, str) or OPAQUE_ID_RE.fullmatch(store) is None:
        raise EvidenceDigestError(f"{label}.store_profile_ref is invalid")
    if not isinstance(artifact_id, str) or OPAQUE_ID_RE.fullmatch(artifact_id) is None:
        raise EvidenceDigestError(f"{label}.artifact_id is invalid")
    if not _is_u64(size):
        raise EvidenceDigestError(f"{label}.bytes is not u64")
    if not isinstance(digest, str) or SHA256_RE.fullmatch(digest) is None:
        raise EvidenceDigestError(f"{label}.sha256 is invalid")
    return {
        "store_profile_ref": store,
        "artifact_id": artifact_id,
        "bytes": size,
        "sha256": digest,
    }


def normalize_evidence_record(value: Any, index: int) -> dict[str, Any]:
    label = f"evidence[{index}]"
    if not isinstance(value, dict):
        raise EvidenceDigestError(f"{label} must be an object")
    _require_exact_keys(value, EXPECTED_FIELDS, label)
    requirement = value.get("requirement_id")
    evidence_class = value.get("evidence_class")
    artifact = validate_artifact_ref(value.get("artifact_ref"), f"{label}.artifact_ref")
    artifact_sha = value.get("artifact_sha256")
    raw_digest = value.get("raw_outcome_digest")
    availability = value.get("availability")
    if not isinstance(requirement, str) or OPAQUE_ID_RE.fullmatch(requirement) is None:
        raise EvidenceDigestError(f"{label}.requirement_id is invalid")
    if not isinstance(evidence_class, str) or CLOSED_ENUM_RE.fullmatch(evidence_class) is None:
        raise EvidenceDigestError(f"{label}.evidence_class is invalid")
    if not isinstance(artifact_sha, str) or SHA256_RE.fullmatch(artifact_sha) is None:
        raise EvidenceDigestError(f"{label}.artifact_sha256 is invalid")
    if artifact_sha != artifact["sha256"]:
        raise EvidenceDigestError(f"{label}.artifact_sha256 differs from artifact_ref.sha256")
    if not isinstance(raw_digest, str) or SHA256_RE.fullmatch(raw_digest) is None:
        raise EvidenceDigestError(f"{label}.raw_outcome_digest is invalid")
    if not isinstance(availability, str) or CLOSED_ENUM_RE.fullmatch(availability) is None:
        raise EvidenceDigestError(f"{label}.availability is invalid")
    return {
        "requirement_id": requirement,
        "evidence_class": evidence_class,
        "artifact_ref": artifact,
        "artifact_sha256": artifact_sha,
        "raw_outcome_digest": raw_digest,
        "availability": availability,
    }


def render_evidence_manifest(evidence: Any) -> bytes:
    if not isinstance(evidence, list):
        raise EvidenceDigestError("evidence must be an array")
    if len(evidence) > MAX_EVIDENCE:
        raise EvidenceDigestError(f"evidence count exceeds {MAX_EVIDENCE}")
    normalized = [normalize_evidence_record(item, index) for index, item in enumerate(evidence)]
    requirement_ids = [item["requirement_id"] for item in normalized]
    if len(set(requirement_ids)) != len(requirement_ids):
        raise EvidenceDigestError("evidence requirement_id values must be unique")
    output = bytearray(MAGIC)
    for item in normalized:
        output.extend(canonical_json_bytes(item))
    if len(output) > MAX_MANIFEST_BYTES:
        raise EvidenceDigestError(f"evidence manifest exceeds {MAX_MANIFEST_BYTES} bytes")
    return bytes(output)


def accepted_evidence_digest(evidence: Any) -> str:
    return hashlib.sha256(render_evidence_manifest(evidence)).hexdigest()


def result_record(evidence: Any) -> dict[str, Any]:
    manifest = render_evidence_manifest(evidence)
    return {
        "schema_version": SCHEMA_VERSION,
        "profile": PROFILE,
        "record_count": len(evidence),
        "manifest_bytes": len(manifest),
        "evidence_digest": hashlib.sha256(manifest).hexdigest(),
    }
