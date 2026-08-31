#!/usr/bin/env python3
"""Validate the non-authoritative context artifact candidate builder v1."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Any, Iterable, Mapping

TOOLS_DIR = Path(__file__).resolve().parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

from context_artifact_builder_v1.build import build_candidate, write_candidate  # noqa: E402
from context_artifact_builder_v1.bundle import parse_bundle  # noqa: E402
from context_artifact_builder_v1.core import (  # noqa: E402
    ADDITIONAL_FAILURE_CODES,
    ARTIFACT_FORMAT,
    ARTIFACT_ROOT,
    AUTHORITY_FIELDS,
    RECORD_KIND,
    SCHEMA_VERSION,
    STATUS,
    UNRESOLVED_MANIFEST_FIELDS,
    assert_candidate_digest,
)

EXPECTED_MODULES = (
    "tools/context_artifact_builder_v1/__init__.py",
    "tools/context_artifact_builder_v1/core.py",
    "tools/context_artifact_builder_v1/bundle.py",
    "tools/context_artifact_builder_v1/extract.py",
    "tools/context_artifact_builder_v1/build.py",
)
EXPECTED_CASES = tuple(f"CAC1-{index:03d}" for index in range(1, 21))
EXPECTED_PATHS = {
    "contract": "docs/handoff/CONTEXT_ARTIFACT_CANDIDATE_V1.md",
    "digest_contract": "docs/handoff/CONTEXT_ARTIFACT_CANDIDATE_DIGEST_V1.md",
    "index": "docs/handoff/CONTEXT_ARTIFACT_CANDIDATE_INDEX.md",
    "candidate_schema": "swarm/context-artifact-candidate-schema-v1.toml",
    "digest_profile": "swarm/context-artifact-candidate-digest-v1.toml",
    "implementation": "tools/build-context-artifact-candidate.py",
    "powershell_wrapper": "tools/build-context-artifact-candidate.ps1",
    "structural_validator": "tools/validate-context-artifact-candidate.py",
    "structural_validator_wrapper": "tools/validate-context-artifact-candidate.ps1",
    "qualification_readme": "qualification/context-artifact/README.md",
    "qualification_cases": "qualification/context-artifact/cases-v1.toml",
    "qualification_tests": "qualification/context-artifact/test_context_artifact_candidate_v1.py",
    "manual_workflow": ".github/workflows/context-artifact-candidate.yml",
}


class Validation:
    def __init__(self) -> None:
        self.checks: list[dict[str, str]] = []
        self.errors: list[str] = []

    def require(self, condition: bool, check_id: str, detail: str) -> None:
        status = "PASS" if condition else "FAIL"
        self.checks.append({"id": check_id, "status": status, "detail": detail})
        if not condition:
            self.errors.append(f"{check_id}: {detail}")


def load_toml(root: Path, relative: str, validation: Validation) -> dict[str, Any]:
    path = root / relative
    try:
        with path.open("rb") as handle:
            value = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        validation.require(False, f"file:{relative}", f"unable to load TOML: {exc}")
        return {}
    validation.require(isinstance(value, dict), f"file:{relative}", "TOML root is a table")
    return value if isinstance(value, dict) else {}


def strings(value: Any) -> tuple[str, ...]:
    if not isinstance(value, list) or any(not isinstance(item, str) for item in value):
        return ()
    return tuple(value)


def tagged_head(root: Path) -> str:
    object_format = subprocess.run(
        ["git", "-C", str(root), "rev-parse", "--show-object-format"],
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout.strip()
    oid = subprocess.run(
        ["git", "-C", str(root), "rev-parse", "HEAD"],
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout.strip()
    return f"{object_format}:{oid}"


def text_contains(root: Path, relative: str, tokens: Iterable[str], validation: Validation) -> None:
    try:
        text = (root / relative).read_text(encoding="utf-8")
    except OSError as exc:
        validation.require(False, f"text:{relative}", f"unable to read: {exc}")
        return
    for token in tokens:
        validation.require(token in text, f"text:{relative}:{token}", f"contains {token}")


def build_report(root: Path) -> dict[str, Any]:
    validation = Validation()
    registry = load_toml(root, "swarm/context-artifact-builder-v1.toml", validation)
    schema = load_toml(root, "swarm/context-artifact-candidate-schema-v1.toml", validation)
    digest = load_toml(root, "swarm/context-artifact-candidate-digest-v1.toml", validation)
    cases = load_toml(root, "qualification/context-artifact/cases-v1.toml", validation)

    validation.require(
        registry.get("schema_version") == 1
        and registry.get("component") == "context_artifact_builder_v1"
        and registry.get("status") == "EXECUTABLE_CANDIDATE_BUILDER_ONLY",
        "registry-identity",
        "builder registry identity is exact",
    )
    for key, expected in EXPECTED_PATHS.items():
        value = registry.get(key)
        validation.require(value == expected, f"registry-path:{key}", f"{key} = {expected}")
        validation.require((root / expected).is_file(), f"registry-file:{key}", f"registered file exists: {expected}")
    validation.require(registry.get("artifact_root") == ARTIFACT_ROOT, "registry-root", "artifact root is exact")
    validation.require(registry.get("artifact_format") == ARTIFACT_FORMAT, "registry-format", "artifact format is exact")
    validation.require(strings(registry.get("implementation_modules")) == EXPECTED_MODULES, "registry-modules", "implementation module set/order is exact")
    for module in EXPECTED_MODULES:
        validation.require((root / module).is_file(), f"module:{module}", f"module exists: {module}")

    authority = registry.get("authority")
    validation.require(isinstance(authority, dict), "registry-authority", "authority table exists")
    if isinstance(authority, dict):
        validation.require(set(authority) == set(AUTHORITY_FIELDS), "registry-authority-keys", "authority key set is closed")
        validation.require(all(authority.get(key) is False for key in AUTHORITY_FIELDS), "registry-authority-false", "all authority flags are false")

    validation.require(
        schema.get("schema_version") == SCHEMA_VERSION
        and schema.get("record_kind") == RECORD_KIND
        and schema.get("status") == STATUS,
        "schema-identity",
        "candidate schema identity is exact",
    )
    validation.require(schema.get("artifact_format") == ARTIFACT_FORMAT, "schema-format", "schema artifact format is exact")
    validation.require(strings(schema.get("additional_failure_codes")) == ADDITIONAL_FAILURE_CODES, "schema-failures", "additional failure registry is exact")
    validation.require(strings(schema.get("required_unresolved_manifest_fields")) == UNRESOLVED_MANIFEST_FIELDS, "schema-unresolved", "unresolved manifest field set/order is exact")
    schema_authority = schema.get("authority")
    validation.require(isinstance(schema_authority, dict), "schema-authority", "schema authority table exists")
    if isinstance(schema_authority, dict):
        validation.require(set(schema_authority) == set(AUTHORITY_FIELDS), "schema-authority-keys", "schema authority keys are closed")
        validation.require(all(schema_authority.get(key) is False for key in AUTHORITY_FIELDS), "schema-authority-false", "schema authority flags are false")

    validation.require(
        digest.get("schema_version") == 1
        and digest.get("profile") == "context_artifact_candidate_digest_v1",
        "digest-identity",
        "digest profile identity is exact",
    )
    for key in (
        "self_referential_digest_allowed",
        "placeholder_replacement_allowed",
        "parsed_reserialization_allowed",
        "candidate_id_is_context_id",
        "candidate_id_is_materialize_context_operation_id",
        "candidate_sha256_is_control_record_digest",
        "artifact_sha256_is_immutable_artifact_ref",
    ):
        validation.require(digest.get(key) is False, f"digest:{key}", f"{key} is false")

    validation.require(cases.get("case_count") == 20, "cases-count", "twenty cases")
    case_rows = cases.get("case")
    case_ids = tuple(row.get("id") for row in case_rows) if isinstance(case_rows, list) else ()
    validation.require(case_ids == EXPECTED_CASES, "cases-ids", "case IDs are exact and ordered")

    text_contains(
        root,
        "docs/handoff/CONTEXT_ARTIFACT_CANDIDATE_V1.md",
        (
            "working tree",
            "ELIOT_SWARM_CONTEXT_1",
            "identity.context_id",
            "artifact.ref",
            "CANDIDATE_OUTPUT_CONFLICT",
            "does not permit implementation",
        ),
        validation,
    )
    text_contains(
        root,
        "docs/handoff/CONTEXT_ARTIFACT_CANDIDATE_DIGEST_V1.md",
        (
            "candidate_id",
            "candidate_sha256",
            "fixed-point",
        ),
        validation,
    )

    actual = None
    try:
        actual_build = build_candidate(
            root,
            "search-contracts",
            tagged_head(root),
            (),
            f"{ARTIFACT_ROOT}/validation",
        )
        write_candidate(root, actual_build)
        actual = actual_build.candidate
        preamble, blocks = parse_bundle(actual_build.bundle_bytes)
        validation.require(preamble.get("package") == "search-contracts", "actual-package", "current candidate is search-contracts")
        validation.require(len(blocks) > 0, "actual-blocks", "current candidate contains framed blocks")
        validation.require(actual.get("status") == STATUS, "actual-status", "current candidate status is exact")
        validation.require(
            set(actual) == set(strings(schema.get("required_top_level_fields"))),
            "actual-fields",
            "current candidate top-level field set is exact",
        )
        validation.require(actual.get("reason_codes") == [], "actual-reasons", "current candidate has no failure reason")
        validation.require(actual.get("control_record_mutations") == [], "actual-mutations", "current candidate has no control mutation")
        validation.require(actual.get("authority") == {key: False for key in AUTHORITY_FIELDS}, "actual-authority", "current candidate authority ceiling is exact")
        validation.require(assert_candidate_digest(actual), "actual-digest", "current candidate metadata digest is exact")
        projection = actual.get("manifest_projection")
        validation.require(isinstance(projection, dict) and projection.get("schema_instance") is False, "actual-projection", "manifest projection is explicitly non-instance")
        validation.require(isinstance(projection, dict) and strings(projection.get("unresolved_fields")) == UNRESOLVED_MANIFEST_FIELDS, "actual-unresolved", "manifest unresolved fields are exact")
        artifact = actual.get("artifact_candidate")
        validation.require(isinstance(artifact, dict) and artifact.get("format") == ARTIFACT_FORMAT, "actual-format", "current bundle format is exact")
        validation.require(isinstance(artifact, dict) and artifact.get("local_file_is_immutable_artifact_ref") is False, "actual-local-ref", "local file is not an immutable artifact ref")
        validation.require(len(actual_build.candidate_bytes) <= 1_048_576, "actual-metadata-size", "candidate metadata is within one MiB")
    except Exception as exc:  # validation tool must report rather than hide the failure
        validation.require(False, "actual-build", f"current candidate build failed: {type(exc).__name__}: {exc}")

    return {
        "schema_version": 1,
        "validator": "context_artifact_candidate_v1",
        "status": "PASS" if not validation.errors else "FAIL",
        "checks": validation.checks,
        "errors": validation.errors,
        "current_candidate_id": actual.get("candidate_id") if isinstance(actual, dict) else "UNAVAILABLE",
        "authoritative_context_materialized": False,
        "context_manifest_created": False,
        "ticket_issued": False,
        "writer_lease_created": False,
        "implementation_authorized": False,
        "package_acceptance_claimed": False,
        "g0_acceptance_claimed": False,
        "w0_acceptance_claimed": False,
        "w1_authority_claimed": False,
    }


def parse_args(argv: Iterable[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".")
    parser.add_argument("--json", action="store_true")
    return parser.parse_args(argv)


def main(argv: Iterable[str] | None = None) -> int:
    args = parse_args(argv)
    root = Path(args.root).resolve()
    if not root.is_dir():
        print(f"repository root does not exist: {root}", file=sys.stderr)
        return 2
    report = build_report(root)
    if args.json:
        print(json.dumps(report, ensure_ascii=False, sort_keys=True, separators=(",", ":")))
    elif report["status"] == "PASS":
        print(f"PASS: {len(report['checks'])} checks; candidate creates no authority")
    else:
        print(f"FAIL: {len(report['errors'])} error(s)", file=sys.stderr)
        for error in report["errors"]:
            print(f"- {error}", file=sys.stderr)
    return 0 if report["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
