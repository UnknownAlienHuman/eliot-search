#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = ROOT / "tools/validate-integration-bootstrap.py"
spec = importlib.util.spec_from_file_location("integration_bootstrap_validator", MODULE_PATH)
assert spec and spec.loader
validator = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = validator
spec.loader.exec_module(validator)


class IntegrationBootstrapValidatorTests(unittest.TestCase):
    def make_repo(self) -> Path:
        root = Path(tempfile.mkdtemp())
        (root / ".cargo").mkdir(parents=True)
        (root / ".github/workflows").mkdir(parents=True)
        (root / "config").mkdir(parents=True)
        (root / "rust-toolchain.toml").write_text(
            '[toolchain]\nchannel = "1.98.0"\nprofile = "minimal"\ncomponents = ["clippy", "rustfmt"]\ntargets = ["x86_64-pc-windows-msvc"]\n',
            encoding="utf-8",
        )
        (root / ".cargo/config.toml").write_text(
            '[alias]\ncheck-all = "check --workspace --all-targets --locked"\n'
            'test-all = "test --workspace --all-targets --locked"\n'
            'clippy-all = "clippy --workspace --all-targets --locked -- -D warnings"\n'
            'doc-all = "doc --workspace --no-deps --locked"\n',
            encoding="utf-8",
        )
        members = ",\n".join(f'  "crate-{index}"' for index in range(45))
        (root / "Cargo.toml").write_text(
            f'[workspace]\nresolver = "3"\nmembers = [\n{members}\n]\n[workspace.package]\nedition = "2024"\n',
            encoding="utf-8",
        )
        (root / "config/build-profiles-v1.toml").write_text(
            'status = "FROZEN_BOOTSTRAP_NOT_PRODUCT_ACCEPTED"\ndefault_profile = "P00_FOUNDATION"\n'
            'automatic_profile_upgrade = false\n'
            + "".join(
                f'[[profile]]\nid = "{profile}"\ndefault = {str(profile == "P00_FOUNDATION").lower()}\n'
                for profile in validator.EXPECTED_PROFILES
            ),
            encoding="utf-8",
        )
        directories = "\n".join(f'{key} = "{value}"' for key, value in validator.EXPECTED_LAYOUT_DIRECTORIES.items())
        (root / "config/data-layout-v1.toml").write_text(
            'root_must_be_dedicated = true\nroot_must_not_be_repository_checkout = true\n'
            'root_must_not_be_source_identity = true\nowner_only_acl_required = true\n'
            'inherited_broad_acl_forbidden = true\nsymlink_or_reparse_escape_forbidden = true\n'
            'plaintext_secret_storage_forbidden = true\n[directories]\n'
            + directories
            + '\n[control]\nredb_role = "CONTROL_JOURNAL_ONLY"\nsearchable_corpus_forbidden = true\n'
            '[qdrant]\nsole_search_index = true\n[runtime]\nunsaved_bytes_must_remain_memory_only = true\n'
            '[migration]\nunknown_outcome_requires_quarantine = true\n',
            encoding="utf-8",
        )
        (root / ".github/workflows/integration-bootstrap.yml").write_text(
            'on:\n  workflow_dispatch:\npermissions:\n  contents: read\nsteps:\n  persist-credentials: false\n',
            encoding="utf-8",
        )
        return root

    def test_valid_preview_without_lock(self) -> None:
        root = self.make_repo()
        self.assertEqual([], validator.validate(root, allow_missing_lock=True))

    def test_missing_lock_fails_verification(self) -> None:
        root = self.make_repo()
        codes = {finding.code for finding in validator.validate(root, allow_missing_lock=False)}
        self.assertIn("CARGO_LOCK_MISSING", codes)

    def test_toolchain_drift_fails(self) -> None:
        root = self.make_repo()
        (root / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "stable"\n', encoding="utf-8")
        codes = {finding.code for finding in validator.validate(root, allow_missing_lock=True)}
        self.assertIn("TOOLCHAIN_NOT_EXACT", codes)

    def test_automatic_workflow_trigger_fails(self) -> None:
        root = self.make_repo()
        workflow = root / ".github/workflows/integration-bootstrap.yml"
        workflow.write_text(workflow.read_text(encoding="utf-8") + "push:\n", encoding="utf-8")
        codes = {finding.code for finding in validator.validate(root, allow_missing_lock=True)}
        self.assertIn("BOOTSTRAP_WORKFLOW_AUTOMATIC_TRIGGER", codes)

    def test_redb_search_role_fails(self) -> None:
        root = self.make_repo()
        layout = root / "config/data-layout-v1.toml"
        layout.write_text(layout.read_text(encoding="utf-8").replace("CONTROL_JOURNAL_ONLY", "SEARCH_INDEX"), encoding="utf-8")
        codes = {finding.code for finding in validator.validate(root, allow_missing_lock=True)}
        self.assertIn("REDB_ROLE_INVALID", codes)

    def test_optional_profile_cannot_be_default(self) -> None:
        root = self.make_repo()
        profiles = root / "config/build-profiles-v1.toml"
        profiles.write_text(profiles.read_text(encoding="utf-8").replace('id = "OPTIONAL_DEPTH"\ndefault = false', 'id = "OPTIONAL_DEPTH"\ndefault = true'), encoding="utf-8")
        codes = {finding.code for finding in validator.validate(root, allow_missing_lock=True)}
        self.assertIn("BUILD_PROFILE_DEFAULT_NOT_UNIQUE", codes)


if __name__ == "__main__":
    unittest.main()
