from __future__ import annotations

import json
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TOOLS = ROOT / "tools"
if str(TOOLS) not in sys.path:
    sys.path.insert(0, str(TOOLS))

from context_artifact_builder_v1.bundle import render_bundle
from context_artifact_builder_v1.core import (
    ARTIFACT_FORMAT,
    BundleBlock,
    authority_map as candidate_authority,
    candidate_id,
    candidate_metadata_digest,
)
from context_materialization_planner_v1.core import MaterializationPlanError
from context_materialization_planner_v1.plan import build_plan, write_plan
from ticket_issuance_planner_v2.core import canonical_json_bytes, exact_sha256


def source_record() -> dict:
    return {
        "order": 0,
        "repository_path": "AGENTS.md",
        "git_blob_id": "sha1:" + "1" * 40,
        "exact_sha256": "2" * 64,
        "exact_bytes": 7,
        "materialization": "UTF8_LF",
        "materialized_sha256": exact_sha256(b"# root\n"),
        "materialized_bytes": 7,
    }


def fragment_record() -> dict:
    content = canonical_json_bytes({"registry_path": "swarm/crates.toml", "selector": "package[name=search-contracts]", "value": {"name": "search-contracts"}})
    return {
        "order": 0,
        "registry_path": "swarm/crates.toml",
        "selector": "package[name=search-contracts]",
        "source_git_blob_id": "sha1:" + "3" * 40,
        "source_exact_sha256": "4" * 64,
        "selector_match_count": 1,
        "fragment_sha256": exact_sha256(content),
        "fragment_bytes": len(content),
    }, content


def handoff_content() -> bytes:
    text = '''schema_version = 1
record_kind = "package_handoff_v1"
status = "ACCEPTED"

[identity]
handoff_id = "contracts-001"
operation_id = "1111111111111111111111111111111111111111111111111111111111111111"
package = "search-contracts"
stage = "W0"
accepted_at = "2026-08-31T00:00:00Z"

[accepted_code]
base_commit = "sha1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
final_commit = "sha1:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
changed_files_digest = "2222222222222222222222222222222222222222222222222222222222222222"

[public_surface]
api_schema_digest = "3333333333333333333333333333333333333333333333333333333333333333"
configuration_digest = { state = "ABSENT", value = "" }
error_reason_digest = "4444444444444444444444444444444444444444444444444444444444444444"

[[evidence]]
requirement_id = "contract-test"
evidence_class = "CONTRACT_TEST"
artifact_ref = { store_profile_ref = "qualified-store-v1", artifact_id = "evidence-1", bytes = 1, sha256 = "5555555555555555555555555555555555555555555555555555555555555555" }
artifact_sha256 = "5555555555555555555555555555555555555555555555555555555555555555"
raw_outcome_digest = "6666666666666666666666666666666666666666666666666666666666666666"
availability = "AVAILABLE"

[compatibility]
class = "COMPATIBLE"
'''
    return text.encode("utf-8")


class Fixture:
    def __init__(self, with_handoff: bool = False) -> None:
        self.tmp = tempfile.TemporaryDirectory(prefix="materialization-plan-")
        self.root = Path(self.tmp.name)
        self.base_commit = "sha1:" + "a" * 40
        source = source_record()
        fragment, fragment_content = fragment_record()
        blocks = [
            BundleBlock("source", "--- repository-path: AGENTS.md ---", source, b"# root\n"),
            BundleBlock("registry_fragment", "--- registry-selector: swarm/crates.toml::package[name=search-contracts] ---", fragment, fragment_content),
        ]
        handoffs = []
        if with_handoff:
            raw = handoff_content()
            handoff = {
                "package": "search-contracts",
                "path": "swarm/handoffs/search-contracts/contracts-001.toml",
                "handoff_id": "contracts-001",
                "git_blob_id": "sha1:" + "7" * 40,
                "exact_record_file_sha256": exact_sha256(raw),
                "accepted_commit": "sha1:" + "b" * 40,
                "api_schema_digest": "3" * 64,
                "error_reason_digest": "4" * 64,
                "order": 0,
                "materialization": "EXACT_UTF8_LF",
                "materialized_sha256": exact_sha256(raw),
                "materialized_bytes": len(raw),
            }
            handoffs.append(handoff)
            blocks.append(BundleBlock("accepted_handoff", "--- accepted-handoff: search-contracts ---", handoff, raw))
        preamble = {
            "artifact_format": ARTIFACT_FORMAT,
            "repository": "UnknownAlienHuman/eliot-search",
            "base_commit": self.base_commit,
            "package": "search-contracts",
            "package_path": "crates/search-contracts",
            "stage": "W0",
            "phase": "P00",
            "wave": 0,
            "context_draft_path": "swarm/context-drafts/p00/search-contracts.toml",
            "context_draft_git_blob_id": "sha1:" + "8" * 40,
            "context_draft_exact_sha256": "9" * 64,
            "source_count": 1,
            "registry_fragment_count": 1,
            "accepted_handoff_count": len(handoffs),
            "required_unavailable_checks": ["real-toolchain"],
        }
        self.bundle = render_bundle(preamble, blocks)
        identifier = candidate_id(self.bundle)
        bundle_rel = f"artifacts/context-artifact-candidates/search-contracts/{identifier}.context"
        candidate_rel = f"artifacts/context-artifact-candidates/search-contracts/{identifier}.json"
        candidate = {
            "schema_version": 1,
            "record_kind": "context_artifact_candidate_v1",
            "status": "ARTIFACT_CANDIDATE_NOT_STORED_NOT_SIGNED",
            "candidate_id": identifier,
            "repository": {"name": "UnknownAlienHuman/eliot-search", "base_commit": self.base_commit, "working_tree_used_as_input": False},
            "package": {"name": "search-contracts", "path": "crates/search-contracts", "stage": "W0", "phase": "P00", "wave": 0},
            "draft": {"path": preamble["context_draft_path"], "git_blob_id": preamble["context_draft_git_blob_id"], "exact_file_sha256": preamble["context_draft_exact_sha256"], "source_ceiling_class": "P00_EXACT_CONTRACT_PACK"},
            "artifact_candidate": {"relative_path": bundle_rel, "sha256": exact_sha256(self.bundle), "bytes": len(self.bundle), "format": ARTIFACT_FORMAT, "local_file_is_immutable_artifact_ref": False},
            "candidate_metadata_path": candidate_rel,
            "sources": [source],
            "registry_fragments": [fragment],
            "accepted_handoffs": handoffs,
            "required_unavailable_checks": ["real-toolchain"],
            "preflight_checks": [],
            "reason_codes": [],
            "verification": {"source_count": 1, "registry_fragment_count": 1, "accepted_handoff_count": len(handoffs), "forbidden_path_scan_passed": True, "bundle_roundtrip_verified": True, "local_output_readback_required": True, "authoritative_artifact_store_readback_verified": False},
            "manifest_projection": {"target_record_kind": "context_manifest_v1", "schema_instance": False, "status": "PROJECTION_REQUIRES_EXTERNAL_STORE_DUAL_SIGNATURE_AND_COMMIT", "known": {}, "unresolved_fields": []},
            "ordinary_artifact_writes": [bundle_rel, candidate_rel],
            "control_record_mutations": [],
            "authority": candidate_authority(),
        }
        candidate["candidate_sha256"] = candidate_metadata_digest(candidate)
        self.bundle_rel = bundle_rel
        self.candidate_rel = candidate_rel
        (self.root / bundle_rel).parent.mkdir(parents=True, exist_ok=True)
        (self.root / bundle_rel).write_bytes(self.bundle)
        (self.root / candidate_rel).parent.mkdir(parents=True, exist_ok=True)
        (self.root / candidate_rel).write_bytes(canonical_json_bytes(candidate))

    def close(self) -> None:
        self.tmp.cleanup()

    def selection(self, signatures: str = "absent") -> str:
        artifact = {"store_profile_ref": "qualified-store-v1", "artifact_id": "context-001", "bytes": len(self.bundle), "sha256": exact_sha256(self.bundle)}
        absent = {"state": "ABSENT", "value": ""}
        value = {
            "context_id": "context-001",
            "created_at": "2026-08-31T00:00:00Z",
            "materializer_identity": "actor:integration:materializer-001",
            "reviewer_identity": "actor:reviewer:context-001",
            "artifact_ref": artifact,
            "artifact_readback": {"verified": True, "verifier_identity": "actor:reviewer:store-001", "verified_at": "2026-08-31T00:00:00Z", "sha256": artifact["sha256"], "bytes": artifact["bytes"]},
            "materializer_signature_ref": absent,
            "reviewer_signature_ref": absent,
        }
        rel = "artifacts/context-materialization-inputs/search-contracts.json"
        target = self.root / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        if signatures == "present":
            first = build_plan(self.root, self.candidate_rel, selection_path=None)
            # Build once with selection written as absent to obtain the payload digest.
            target.write_bytes(canonical_json_bytes(value))
            unsigned = build_plan(self.root, self.candidate_rel, selection_path=rel)
            digest = unsigned.plan["prospective_manifest"]["signed_payload_sha256"]
            def signature(actor: str, suffix: str) -> dict:
                return {"state": "PRESENT", "value": {"approval_profile_ref": "qualified-approval-v1", "approval_artifact_ref": {"store_profile_ref": "qualified-approval-store-v1", "artifact_id": f"approval-{suffix}", "bytes": 1, "sha256": suffix * 64}, "signed_payload_sha256": digest, "actor_identity": actor}}
            value["materializer_signature_ref"] = signature(value["materializer_identity"], "c")
            value["reviewer_signature_ref"] = signature(value["reviewer_identity"], "d")
        elif signatures == "partial":
            value["materializer_signature_ref"] = {"state": "PRESENT", "value": {"approval_profile_ref": "qualified-approval-v1", "approval_artifact_ref": {"store_profile_ref": "qualified-approval-store-v1", "artifact_id": "approval-c", "bytes": 1, "sha256": "c" * 64}, "signed_payload_sha256": "e" * 64, "actor_identity": value["materializer_identity"]}}
        target.write_bytes(canonical_json_bytes(value))
        return rel


class ContextMaterializationPlanTests(unittest.TestCase):
    def setUp(self) -> None:
        self.fx = Fixture()

    def tearDown(self) -> None:
        self.fx.close()

    def test_missing_selection_is_blocked(self) -> None:
        build = build_plan(self.fx.root, self.fx.candidate_rel)
        self.assertEqual(build.plan["decision"], "BLOCKED_MISSING_EXTERNAL_INPUT")
        self.assertEqual(build.plan["control_record_mutations"], [])
        self.assertTrue(all(value is False for value in build.plan["authority"].values()))

    def test_absent_signatures_produce_payload(self) -> None:
        selection = self.fx.selection("absent")
        build = build_plan(self.fx.root, self.fx.candidate_rel, selection_path=selection)
        self.assertEqual(build.plan["decision"], "READY_FOR_DUAL_SIGNATURE_COLLECTION")
        self.assertIsNotNane(build.payload_bytes)
        self.assertIsNone(build.manifest_bytes)
        self.assertEqual(exact_sha256(build.payload_bytes), build.plan["prospective_manifest"]["signed_payload_sha256"])

    def test_present_signatures_produce_full_proposal(self) -> None:
        selection = self.fx.selection("present")
        build = build_plan(self.fx.root, self.fx.candidate_rel, selection_path=selection)
        self.assertEqual(build.plan["decision"], "READY_FOR_INTEGRATION_OWNER_READBACK_AND_COMMIT")
        self.assertIsNotNane(build.manifest_bytes)
        parsed = tomlib.loads(build.manifest_bytes.decode("utf-8"))
        self.assertEqual(parsed["status"], "MATERIALIZED")
        self.assertEqual(parsed["signature"]["record_sha256"], build.plan["prospective_manifest"]["signed_payload_sha256"])
        self.assertTrue(build.plan["prospective_manifest"]["target_control_record_path"].endswith(".toml"))

    def test_partial_signature_set_is_blocked(self) -> None:
        selection = self.fx.selection("partial")
        build = build_plan(self.fx.root, self.fx.candidate_rel, selection_path=selection)
        self.assertEqual(build.plan["decision"], "BLOCKED_PARTIAL_SIGNATURE_SET")
        self.assertIsNone(build.manifest_bytes)

    def test_actor_conflict_rejected(self) -> None:
        selection = self.fx.selection()
        path = self.fx.root / selection
        value = json.loads(path.read_text())
        value["reviewer_identity"] = value["materializer_identity"]
        path.write_bytes(canonical_json_bytes(value))
        with self.assertRaises(MaterializationPlanError):
            build_plan(self.fx.root, self.fx.candidate_rel, selection_path=selection)

    def test_artifact_mismatch_rejected(self) -> None:
        selection = self.fx.selection()
        path = self.fx.root / selection
        value = json.loads(path.read_text())
        value["artifact_ref"]["sha256"] = "f" * 64
        path.write_bytes(canonical_json_bytes(value))
        with self.assertRaises(MaterializationPlanError):
            build_plan(self.fx.root, self.fx.candidate_rel, selection_path=selection)

    def test_signature_payload_mismatch_rejected(self) -> None:
        selection = self.fx.selection("present")
        path = self.fx.root / selection
        value = json.loads(path.read_text())
        value["reviewer_signature_ref"]["value"]["signed_payload_sha256"] = "f" * 64
        path.write_bytes(canonical_json_bytes(value))
        with self.assertRaises(MaterializationPlanError):
            build_plan(self.fx.root, self.fx.candidate_rel, selection_path=selection)

    def test_candidate_digest_mismatch_rejected(self) -> None:
        path = self.fx.root / self.fx.candidate_rel
        value = json.loads(path.read_text())
        value["candidate_id"] = "0" * 64
        path.write_bytes(canonical_json_bytes(value))
        with self.assertRaises(MaterializationPlanError):
            build_plan(self.fx.root, self.fx.candidate_rel)

    def test_bundle_mismatch_rejected(self) -> None:
        path = self.fx.root / self.fx.bundle_rel
        path.write_bytes(path.read_bytes() + b"x")
        with self.assertRaises(MaterializationPlanError):
            build_plan(self.fx.root, self.fx.candidate_rel)

    def test_write_is_idempotent_and_conflict_detected(self) -> None:
        selection = self.fx.selection("present")
        build = build_plan(self.fx.root, self.fx.candidate_rel, selection_path=selection)
        first = write_plan(self.fx.root, build)
        second = write_plan(self.fx.root, build)
        self.assertEqual(first, second)
        first[0].write_bytes(b"different\n")
        with self.assertRaises(MaterializationPlanError):
            write_plan(self.fx.root, build)

    def test_accepted_handoff_projection_derives_evidence_digest(self) -> None:
        self.fx.close()
        self.fx = Fixture(with_handoff=True)
        selection = self.fx.selection("absent")
        build = build_plan(self.fx.root, self.fx.candidate_rel, selection_path=selection)
        projections = build.plan["accepted_handoff_projections"]
        self.assertEqual(len(projections), 1)
        self.assertRegex(projections[0]["evidence_digest"], r"^[0-9a-f]{64}$")

    def test_operation_id_excludes_signature_artifacts(self) -> None:
        selection = self.fx.selection("absent")
        first = build_plan(self.fx.root, self.fx.candidate_rel, selection_path=selection)
        selection = self.fx.selection("present")
        second = build_plan(self.fx.root, self.fx.candidate_rel, selection_path=selection)
        self.assertEqual(first.plan["operation"]["operation_id"], second.plan["operation"]["operation_id"])
        self.assertEqual(first.plan["prospective_manifest"]["signed_payload_sha256"], second.plan["prospective_manifest"]["signed_payload_sha256"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
