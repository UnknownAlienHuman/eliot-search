from __future__ import annotations

import sys
import unittest
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
TOOLS_DIR = REPOSITORY_ROOT / "tools"
FIXTURE_DIR = REPOSITORY_ROOT / "qualification" / "ticket-issuance"
for path in (str(TOOLS_DIR), str(FIXTURE_DIR)):
    if path not in sys.path:
        sys.path.insert(0, path)

from context_artifact_builder_v1.build import build_candidate, write_candidate
from context_artifact_builder_v1.bundle import parse_bundle
from context_artifact_builder_v1.core import (
    ARTIFACT_FORMAT,
    ARTIFACT_ROOT,
    CandidateFailure,
    UNRESOLVED_MANIFEST_FIELDS,
    assert_candidate_digest,
    authority_map,
)
from fixture_plan_ticket_issuance_v2 import FixtureRepository


def install_builder_contract(repo: FixtureRepository) -> None:
    repo.write(
        "swarm/context-artifact-builder-v1.toml",
        '''schema_version = 1
component = "context_artifact_builder_v1"
candidate_schema = "swarm/context-artifact-candidate-schema-v1.toml"
digest_profile = "swarm/context-artifact-candidate-digest-v1.toml"
artifact_root = "artifacts/context-artifact-candidates"
artifact_format = "ELIOT_SWARM_CONTEXT_1"
[authority]
materializes_authoritative_context = false
creates_context_manifest_record = false
creates_immutable_artifact_ref = false
creates_assignment_ticket = false
creates_writer_lease = false
authorizes_implementation = false
publishes_package_handoff = false
accepts_gate_or_wave = false
advances_launch_state = false
''',
    )
    repo.write(
        "swarm/context-artifact-candidate-schema-v1.toml",
        '''schema_version = 1
record_kind = "context_artifact_candidate_v1"
artifact_format = "ELIOT_SWARM_CONTEXT_1"
''',
    )
    repo.write(
        "swarm/context-artifact-candidate-digest-v1.toml",
        '''schema_version = 1
profile = "context_artifact_candidate_digest_v1"
self_referential_digest_allowed = false
''',
    )
    repo.write("artifacts/context-artifact-candidates/README.md", "# candidates\n")
    repo.write("artifacts/context-artifact-candidates/.gitignore", "*.context\n*.json\n")
    repo.commit("install context artifact builder contract")


class ContextArtifactCandidateV1Tests(unittest.TestCase):
    def setUp(self) -> None:
        self.repo = FixtureRepository()
        install_builder_contract(self.repo)

    def tearDown(self) -> None:
        self.repo.close()

    def build(self, package: str = "search-contracts", **kwargs: object):
        return build_candidate(
            self.repo.root,
            package,
            str(kwargs.pop("base_commit", self.repo.tagged_head)),
            kwargs.pop("accepted_handoff_paths", ()),
            str(kwargs.pop("output_root", ARTIFACT_ROOT)),
        )

    def assert_failure(self, code: str, function) -> CandidateFailure:
        with self.assertRaises(CandidateFailure) as raised:
            function()
        self.assertEqual(raised.exception.reason_code, code)
        return raised.exception

    def test_01_ready_search_contracts_candidate(self) -> None:
        build = self.build()
        self.assertEqual(build.candidate["status"], "ARTIFACT_CANDIDATE_NOT_STORED_NOT_SIGNED")
        self.assertEqual(build.candidate["reason_codes"], [])
        self.assertEqual(build.candidate["artifact_candidate"]["format"], ARTIFACT_FORMAT)
        preamble, blocks = parse_bundle(build.bundle_bytes)
        self.assertEqual(preamble["package"], "search-contracts")
        self.assertEqual(len(blocks), preamble["source_count"] + preamble["registry_fragment_count"])

    def test_02_deterministic_candidate_and_bundle(self) -> None:
        first = self.build()
        second = self.build()
        self.assertEqual(first.bundle_bytes, second.bundle_bytes)
        self.assertEqual(first.candidate_bytes, second.candidate_bytes)
        self.assertEqual(first.candidate["candidate_id"], second.candidate["candidate_id"])

    def test_03_working_tree_is_ignored(self) -> None:
        base = self.repo.tagged_head
        first = self.build(base_commit=base)
        self.repo.write("AGENTS.md", "# uncommitted change\n")
        second = self.build(base_commit=base)
        self.assertEqual(first.bundle_bytes, second.bundle_bytes)
        self.assertEqual(first.candidate["candidate_id"], second.candidate["candidate_id"])

    def test_04_crlf_source_normalization(self) -> None:
        self.repo.write("AGENTS.md", b"# root\r\nsecond\r\n")
        self.repo.commit("CRLF source")
        build = self.build()
        source = next(item for item in build.candidate["sources"] if item["repository_path"] == "AGENTS.md")
        self.assertNotEqual(source["exact_sha256"], source["materialized_sha256"])
        _preamble, blocks = parse_bundle(build.bundle_bytes)
        block = next(item for item in blocks if item.metadata.get("repository_path") == "AGENTS.md")
        self.assertEqual(block.content, b"# root\nsecond\n")

    def test_05_lone_cr_source_normalization(self) -> None:
        self.repo.write("AGENTS.md", b"a\rb\r")
        self.repo.commit("lone CR source")
        build = self.build()
        _preamble, blocks = parse_bundle(build.bundle_bytes)
        block = next(item for item in blocks if item.metadata.get("repository_path") == "AGENTS.md")
        self.assertEqual(block.content, b"a\nb\n")

    def test_06_length_framing_tolerates_header_text(self) -> None:
        content = b"--- end-context-artifact ---\n--- repository-path: fake ---\n"
        self.repo.write("AGENTS.md", content)
        self.repo.commit("header-like source content")
        build = self.build()
        _preamble, blocks = parse_bundle(build.bundle_bytes)
        block = next(item for item in blocks if item.metadata.get("repository_path") == "AGENTS.md")
        self.assertEqual(block.content, content)

    def test_07_missing_committed_source(self) -> None:
        (self.repo.root / "crates/search-contracts/AGENTS.md").unlink()
        self.repo.commit("remove source")
        self.assert_failure("CONTEXT_SOURCE_MISSING", self.build)

    def test_08_non_utf8_source(self) -> None:
        self.repo.write("AGENTS.md", b"\xff\xfe")
        self.repo.commit("non UTF-8 source")
        self.assert_failure("CONTEXT_SOURCE_NOT_UTF8", self.build)

    def test_09_nul_source(self) -> None:
        self.repo.write("AGENTS.md", b"root\x00value\n")
        self.repo.commit("NUL source")
        self.assert_failure("CONTEXT_SOURCE_CONTAINS_NUL", self.build)

    def test_10_unsupported_selector(self) -> None:
        path = "swarm/context-drafts/p00/search-contracts.toml"
        self.repo.replace(path, "authorized_packages[search-contracts]", "authorized_package[search-contracts]")
        self.repo.commit("invalid selector")
        self.assert_failure("CONTEXT_SELECTOR_INVALID", self.build)

    def test_11_nonunique_selector(self) -> None:
        self.repo.replace(
            "swarm/launch-state.toml",
            'authorized_packages = ["search-contracts"]',
            'authorized_packages = ["search-contracts", "search-contracts"]',
        )
        self.repo.commit("duplicate launch membership")
        self.assert_failure("CONTEXT_SELECTOR_NOT_UNIQUE", self.build)

    def test_12_conditional_package_requires_handoff(self) -> None:
        self.assert_failure("HANDOFF_SLOT_UNSATISFIED", lambda: self.build("search-domain"))

    def test_13_valid_handoff_is_embedded_exactly(self) -> None:
        handoff, commit = self.repo.add_accepted_contracts_handoff()
        build = self.build(
            "search-domain",
            base_commit=commit,
            accepted_handoff_paths=(handoff,),
        )
        self.assertEqual(len(build.candidate["accepted_handoffs"]), 1)
        raw = (self.repo.root / handoff).read_bytes()
        _preamble, blocks = parse_bundle(build.bundle_bytes)
        block = next(item for item in blocks if item.kind == "accepted_handoff")
        self.assertEqual(block.content, raw)

    def test_14_invalid_handoff_signature(self) -> None:
        handoff, commit = self.repo.add_accepted_contracts_handoff(valid_signature=False)
        self.assert_failure(
            "HANDOFF_RECORD_INVALID",
            lambda: self.build(
                "search-domain",
                base_commit=commit,
                accepted_handoff_paths=(handoff,),
            ),
        )

    def test_15_unexpected_handoff_for_contracts(self) -> None:
        handoff, commit = self.repo.add_accepted_contracts_handoff()
        self.assert_failure(
            "CURRENT_PACKAGE_CONTROL_RECORD_EXISTS",
            lambda: self.build(base_commit=commit, accepted_handoff_paths=(handoff,)),
        )

    def test_16_existing_current_package_record_conflicts(self) -> None:
        self.repo.write("swarm/context-manifests/search-contracts/existing.toml", "record = true\n")
        self.repo.commit("existing context record")
        self.assert_failure("CURRENT_PACKAGE_CONTROL_RECORD_EXISTS", self.build)

    def test_17_output_outside_candidate_root(self) -> None:
        self.assert_failure(
            "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT",
            lambda: self.build(output_root="artifacts/elsewhere"),
        )

    def test_18_equal_output_is_idempotent(self) -> None:
        build = self.build()
        first = write_candidate(self.repo.root, build)
        second = write_candidate(self.repo.root, build)
        self.assertEqual(first, second)
        self.assertEqual(first[0].read_bytes(), build.bundle_bytes)
        self.assertEqual(first[1].read_bytes(), build.candidate_bytes)

    def test_19_different_existing_output_conflicts(self) -> None:
        build = self.build()
        _bundle, metadata = write_candidate(self.repo.root, build)
        metadata.write_bytes(b"different\n")
        self.assert_failure(
            "CANDIDATE_OUTPUT_CONFLICT",
            lambda: write_candidate(self.repo.root, build),
        )

    def test_20_digest_projection_and_authority_ceiling(self) -> None:
        build = self.build()
        candidate = build.candidate
        self.assertTrue(assert_candidate_digest(candidate))
        self.assertEqual(candidate["authority"], authority_map())
        self.assertEqual(candidate["control_record_mutations"], [])
        projection = candidate["manifest_projection"]
        self.assertFalse(projection["schema_instance"])
        self.assertEqual(tuple(projection["unresolved_fields"]), UNRESOLVED_MANIFEST_FIELDS)
        self.assertFalse(candidate["artifact_candidate"]["local_file_is_immutable_artifact_ref"])
        self.assertFalse(candidate["verification"]["authoritative_artifact_store_readback_verified"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
