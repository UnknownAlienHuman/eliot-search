#!/usr/bin/env python3
"""Fail-closed validator for the P00 integration bootstrap.

This validator proves repository structure only. It never issues control records or
claims package, gate, wave, runtime, or product acceptance.
"""

from __future__ import annotations

import argparse
import json
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any

EXPECTED_TOOLCHAIN = {
    "channel": "1.98.0",
    "profile": "minimal",
    "components": ["clippy", "rustfmt"],
    "targets": ["x86_64-pc-windows-msvc"],
}
EXPECTED_PROFILES = (
    "P00_FOUNDATION",
    "DIRECT_BASELINE",
    "LEXICAL_BASELINE",
    "CODE_CURRENT",
    "OPTIONAL_DEPTH",
)
EXPECTED_LAYOUT_DIRECTORIES = {
    "control": "control",
    "objects": "objects",
    "qdrant": "qdrant",
    "runtime": "runtime",
    "temporary": "tmp",
    "backups": "backups",
    "quarantine": "quarantine",
}


@dataclass(frozen=True)
class Finding:
    code: str
    path: str
    detail: str


def load_toml(path: Path) -> dict[str, Any]:
    try:
        with path.open("rb") as handle:
            value = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise ValueError(f"cannot load TOML: {exc}") from exc
    if not isinstance(value, dict):
        raise ValueError("TOML root must be a table")
    return value


def add(findings: list[Finding], code: str, path: Path, detail: str) -> None:
    findings.append(Finding(code=code, path=path.as_posix(), detail=detail))


def validate_toolchain(root: Path, findings: list[Finding]) -> None:
    path = root / "rust-toolchain.toml"
    try:
        toolchain = load_toml(path).get("toolchain")
    except ValueError as exc:
        add(findings, "TOOLCHAIN_INVALID", path, str(exc))
        return
    if toolchain != EXPECTED_TOOLCHAIN:
        add(findings, "TOOLCHAIN_NOT_EXACT", path, f"expected {EXPECTED_TOOLCHAIN!r}, got {toolchain!r}")


def validate_cargo_config(root: Path, findings: list[Finding]) -> None:
    path = root / ".cargo/config.toml"
    try:
        config = load_toml(path)
    except ValueError as exc:
        add(findings, "CARGO_CONFIG_INVALID", path, str(exc))
        return
    aliases = config.get("alias")
    expected = {
        "check-all": "check --workspace --all-targets --locked",
        "test-all": "test --workspace --all-targets --locked",
        "clippy-all": "clippy --workspace --all-targets --locked -- -D warnings",
        "doc-all": "doc --workspace --no-deps --locked",
    }
    if aliases != expected:
        add(findings, "CARGO_ALIASES_NOT_EXACT", path, f"expected {expected!r}, got {aliases!r}")


def validate_build_profiles(root: Path, findings: list[Finding]) -> None:
    path = root / "config/build-profiles-v1.toml"
    try:
        registry = load_toml(path)
    except ValueError as exc:
        add(findings, "BUILD_PROFILE_REGISTRY_INVALID", path, str(exc))
        return
    if registry.get("status") != "FROZEN_BOOTSTRAP_NOT_PRODUCT_ACCEPTED":
        add(findings, "BUILD_PROFILE_STATUS_INVALID", path, "bootstrap status must retain the non-acceptance sentinel")
    if registry.get("default_profile") != "P00_FOUNDATION":
        add(findings, "DEFAULT_PROFILE_INVALID", path, "P00_FOUNDATION must be the sole bootstrap default")
    if registry.get("automatic_profile_upgrade") is not False:
        add(findings, "AUTOMATIC_PROFILE_UPGRADE_ENABLED", path, "automatic profile upgrade must fail closed")
    profiles = registry.get("profile")
    if not isinstance(profiles, list):
        add(findings, "BUILD_PROFILES_MISSING", path, "[[profile]] records are required")
        return
    ids = tuple(item.get("id") for item in profiles if isinstance(item, dict))
    if ids != EXPECTED_PROFILES:
        add(findings, "BUILD_PROFILE_SET_NOT_EXACT", path, f"expected {EXPECTED_PROFILES!r}, got {ids!r}")
    defaults = [item.get("id") for item in profiles if isinstance(item, dict) and item.get("default") is True]
    if defaults != ["P00_FOUNDATION"]:
        add(findings, "BUILD_PROFILE_DEFAULT_NOT_UNIQUE", path, f"unexpected defaults: {defaults!r}")


def validate_data_layout(root: Path, findings: list[Finding]) -> None:
    path = root / "config/data-layout-v1.toml"
    try:
        layout = load_toml(path)
    except ValueError as exc:
        add(findings, "DATA_LAYOUT_INVALID", path, str(exc))
        return
    required_true = (
        "root_must_be_dedicated",
        "root_must_not_be_repository_checkout",
        "root_must_not_be_source_identity",
        "owner_only_acl_required",
        "inherited_broad_acl_forbidden",
        "symlink_or_reparse_escape_forbidden",
        "plaintext_secret_storage_forbidden",
    )
    for field in required_true:
        if layout.get(field) is not True:
            add(findings, "DATA_LAYOUT_FAIL_CLOSED_FIELD", path, f"{field} must be true")
    if layout.get("directories") != EXPECTED_LAYOUT_DIRECTORIES:
        add(findings, "DATA_LAYOUT_DIRECTORY_SET_NOT_EXACT", path, "directory registry differs from the frozen v1 layout")
    control = layout.get("control", {})
    qdrant = layout.get("qdrant", {})
    runtime = layout.get("runtime", {})
    migration = layout.get("migration", {})
    if control.get("redb_role") != "CONTROL_JOURNAL_ONLY" or control.get("searchable_corpus_forbidden") is not True:
        add(findings, "REDB_ROLE_INVALID", path, "redb must remain control-journal-only")
    if qdrant.get("sole_search_index") is not True:
        add(findings, "QDRANT_ROLE_INVALID", path, "Qdrant must remain the sole search/index database")
    if runtime.get("unsaved_bytes_must_remain_memory_only") is not True:
        add(findings, "UNSAVED_BYTES_LAYOUT_INVALID", path, "unsaved bytes must remain memory-only")
    if migration.get("unknown_outcome_requires_quarantine") is not True:
        add(findings, "MIGRATION_UNKNOWN_OUTCOME_UNSAFE", path, "unknown migration outcomes must quarantine")


def validate_workspace(root: Path, findings: list[Finding]) -> None:
    path = root / "Cargo.toml"
    try:
        manifest = load_toml(path)
    except ValueError as exc:
        add(findings, "WORKSPACE_MANIFEST_INVALID", path, str(exc))
        return
    workspace = manifest.get("workspace", {})
    package = manifest.get("workspace", {}).get("package", {})
    if workspace.get("resolver") != "3":
        add(findings, "WORKSPACE_RESOLVER_INVALID", path, "resolver must be 3")
    members = workspace.get("members")
    if not isinstance(members, list) or len(members) != 45 or len(set(members)) != 45:
        add(findings, "WORKSPACE_MEMBER_SET_INVALID", path, "workspace must contain exactly 45 unique packages")
    if package.get("edition") != "2024":
        add(findings, "WORKSPACE_EDITION_INVALID", path, "workspace edition must be 2024")


def validate_lock(root: Path, findings: list[Finding], allow_missing: bool) -> None:
    path = root / "Cargo.lock"
    if not path.is_file():
        if not allow_missing:
            add(findings, "CARGO_LOCK_MISSING", path, "verification mode requires a generated Cargo.lock")
        return
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        add(findings, "CARGO_LOCK_UNREADABLE", path, str(exc))
        return
    if not text.startswith("# This file is automatically @generated by Cargo.\n") or "\nversion = 4\n" not in text:
        add(findings, "CARGO_LOCK_HEADER_INVALID", path, "Cargo.lock must be Cargo-generated format version 4")


def validate_workflow(root: Path, findings: list[Finding]) -> None:
    path = root / ".github/workflows/integration-bootstrap.yml"
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        add(findings, "BOOTSTRAP_WORKFLOW_MISSING", path, str(exc))
        return
    forbidden = ("push:", "pull_request:", "schedule:", "workflow_run:", "workflow_call:")
    if "workflow_dispatch:" not in text:
        add(findings, "BOOTSTRAP_WORKFLOW_NOT_MANUAL", path, "workflow_dispatch is required")
    for trigger in forbidden:
        if trigger in text:
            add(findings, "BOOTSTRAP_WORKFLOW_AUTOMATIC_TRIGGER", path, f"forbidden trigger {trigger}")
    if "contents: read" not in text or "persist-credentials: false" not in text:
        add(findings, "BOOTSTRAP_WORKFLOW_WRITE_CAPABLE", path, "workflow must remain credential-free and read-only")


def validate(root: Path, *, allow_missing_lock: bool) -> list[Finding]:
    findings: list[Finding] = []
    validate_toolchain(root, findings)
    validate_cargo_config(root, findings)
    validate_build_profiles(root, findings)
    validate_data_layout(root, findings)
    validate_workspace(root, findings)
    validate_lock(root, findings, allow_missing_lock)
    validate_workflow(root, findings)
    return findings


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--allow-missing-lock", action="store_true")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)
    findings = validate(args.root.resolve(), allow_missing_lock=args.allow_missing_lock)
    payload = {
        "schema_version": 1,
        "validator": "integration_bootstrap_v1",
        "status": "PASS" if not findings else "FAIL",
        "authority": {
            "issues_control_records": False,
            "accepts_package": False,
            "accepts_gate": False,
            "accepts_wave": False,
            "advances_launch_state": False,
            "claims_runtime_evidence": False,
            "claims_product_acceptance": False,
        },
        "findings": [finding.__dict__ for finding in findings],
    }
    if args.json:
        print(json.dumps(payload, sort_keys=True, separators=(",", ":")))
    else:
        print(f"{payload['status']}: {len(findings)} finding(s)")
        for finding in findings:
            print(f"{finding.code}: {finding.path}: {finding.detail}")
    return 0 if not findings else 1


if __name__ == "__main__":
    sys.exit(main())
