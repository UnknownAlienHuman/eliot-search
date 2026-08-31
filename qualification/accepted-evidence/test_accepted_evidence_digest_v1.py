from __future__ import annotations

import hashlib
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TOOLS = ROOT / "tools"
if str(TOOLS) not in sys.path:
    sys.path.insert(0, str(TOOLS))

from accepted_evidence_digest_v1 import (
    MAGIC,
    EvidenceDigestError,
    accepted_evidence_digest,
    canonical_json_bytes,
    render_evidence_manifest,
    result_record,
)


def record(requirement: str, suffix: str = "1") -> dict:
    artifact_sha = suffix * 64
    return {
        "requirement_id": requirement,
        "evidence_class": "CONTRACT_TEST",
        "artifact_ref": {
            "store_profile_ref": "qualified-store-v1",
            "artifact_id": f"artifact-{requirement}",
            "bytes": 12,
            "sha256": artifact_sha,
        },
        "artifact_sha256": artifact_sha,
        "raw_outcome_digest": "a" * 64,
        "availability": "AVAILABLE",
    }


class AcceptedEvidenceDigestV1Tests(unittest.TestCase):
    def test_empty(self) -> None:
        self.assertEqual(render_evidence_manifest([]), MAGIC)
        self.assertEqual(accepted_evidence_digest([]), hashlib.sha256(MAGIC).hexdigest())

    def test_deterministic(self) -> None:
        value = [record("one"), record("two", "2")]
        self.assertEqual(accepted_evidence_digest(value), accepted_evidence_digest(value))
        self.assertEqual(result_record(value)["record_count"], 2)

    def test_exact_format(self) -> None:
        value = [record("one")]
        expected = MAGIC + canonical_json_bytes(value[0])
        self.assertEqual(render_evidence_manifest(value), expected)

    def test_order_sensitive(self) -> None:
        one = record("one")
        two = record("two", "2")
        self.assertNotEqual(accepted_evidence_digest([one, two]), accepted_evidence_digest([two, one]))

    def test_duplicate_requirement_rejected(self) -> None:
        with self.assertRaises(EvidenceDigestError):
            render_evidence_manifest([record("same"), record("same", "2")])

    def test_unknown_field_rejected(self) -> None:
        value = record("one")
        value["extra"] = True
        with self.assertRaises(EvidenceDigestError):
            render_evidence_manifest([value])

    def test_artifact_digest_mismatch_rejected(self) -> None:
        value = record("one")
        value["artifact_sha256"] = "b" * 64
        with self.assertRaises(EvidenceDigestError):
            render_evidence_manifest([value])

    def test_invalid_identifier_rejected(self) -> None:
        value = record("bad/id")
        with self.assertRaises(EvidenceDigestError):
            render_evidence_manifest([value])

    def test_null_and_float_rejected(self) -> None:
        value = record("one")
        value["artifact_ref"]["bytes"] = 1.5
        with self.assertRaises(EvidenceDigestError):
            render_evidence_manifest([value])
        value = record("one")
        value["availability"] = None
        with self.assertRaises(EvidenceDigestError):
            render_evidence_manifest([value])

    def test_count_bound(self) -> None:
        with self.assertRaises(EvidenceDigestError):
            render_evidence_manifest([record(f"r{i}") for i in range(257)])


if __name__ == "__main__":
    unittest.main(verbosity=2)
