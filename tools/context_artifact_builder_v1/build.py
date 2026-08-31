from __future__ import annotations

from pathlib import Path
from typing import Any, Mapping, Sequence

from ticket_issuance_planner_v2.core import REPOSITORY_NAME, canonical_json_bytes, exact_sha256

from .bundle import parse_bundle, render_bundle
from .core import (
    ARTIFACT_FORMAT,
    ARTIFACT_ROOT,
    RECORD_KIND,
    SCHEMA_VERSION,
    STATUS,
    UNRESOLVED_MANIFEST_FIELDS,
    CandidateBuild,
    CandidateFailure,
    assert_candidate_digest,
    authority_map,
    candidate_id,
    candidate_metadata_digest,
    validate_output_root,
    write_exact_idempotent,
)
from .extract import (
    fragment_records_and_blocks,
    handoff_records_and_blocks,
    preflight,
    source_records_and_blocks,
)


def build_candidate(
    root: str | Path,
    package: str,
    base_commit: str,
    accepted_handoff_paths: Sequence[str] = (),
    output_root: str = ARTIFACT_ROOT,
) -> CandidateBuild:
    root_path = Path(root).resolve()
    output_base = validate_output_root(root_path, output_root)
    view, pair, _launch, package_row, handoff_summaries, checks = preflight(
        str(root_path), package, base_commit, accepted_handoff_paths
    )

    sources, source_blocks = source_records_and_blocks(view, pair)
    fragments, fragment_blocks = fragment_records_and_blocks(view, pair, package)
    handoffs, handoff_blocks = handoff_records_and_blocks(view, handoff_summaries)

    preamble = {
        "artifact_format": ARTIFACT_FORMAT,
        "repository": REPOSITORY_NAME,
        "base_commit": view.tagged_commit,
        "package": package,
        "package_path": package_row.get("path"),
        "stage": pair.ticket.get("stage"),
        "phase": pair.ticket.get("phase"),
        "wave": pair.ticket.get("wave"),
        "context_draft_path": pair.context_path,
        "context_draft_git_blob_id": pair.context_blob,
        "context_draft_exact_sha256": pair.context_sha256,
        "source_count": len(sources),
        "registry_fragment_count": len(fragments),
        "accepted_handoff_count": len(handoffs),
        "required_unavailable_checks": list(pair.unavailable_checks),
    }
    blocks = [*source_blocks, *fragment_blocks, *handoff_blocks]
    bundle_bytes = render_bundle(preamble, blocks)
    parsed_preamble, parsed_blocks = parse_bundle(bundle_bytes)
    if parsed_preamble != preamble or len(parsed_blocks) != len(blocks):
        raise CandidateFailure(
            "BUNDLE_FORMAT_INVALID", "bundle round-trip changed semantic structure"
        )

    identifier = candidate_id(bundle_bytes)
    output_dir = output_base / package
    bundle_path = output_dir / f"{identifier}.context"
    candidate_path = output_dir / f"{identifier}.json"
    bundle_relative = bundle_path.relative_to(root_path).as_posix()
    candidate_relative = candidate_path.relative_to(root_path).as_posix()
    artifact_sha256 = exact_sha256(bundle_bytes)

    manifest_projection = {
        "target_record_kind": "context_manifest_v1",
        "schema_instance": False,
        "status": "PROJECTION_REQUIRES_EXTERNAL_STORE_DUAL_SIGNATURE_AND_COMMIT",
        "known": {
            "identity.package": package,
            "identity.stage": pair.ticket.get("stage"),
            "identity.wave": pair.ticket.get("wave"),
            "identity.base_commit": view.tagged_commit,
            "draft.path": pair.context_path,
            "draft.git_blob_id": pair.context_blob,
            "draft.exact_file_sha256": pair.context_sha256,
            "artifact.sha256": artifact_sha256,
            "artifact.bytes": len(bundle_bytes),
            "artifact.format": ARTIFACT_FORMAT,
            "sources": sources,
            "registry_fragments": fragments,
            "accepted_handoff_inputs": handoffs,
            "verification.source_count": len(sources),
            "verification.registry_fragment_count": len(fragments),
            "verification.accepted_handoff_count": len(handoffs),
            "verification.forbidden_path_scan_passed": True,
        },
        "unresolved_fields": list(UNRESOLVED_MANIFEST_FIELDS),
    }

    candidate: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "record_kind": RECORD_KIND,
        "status": STATUS,
        "candidate_id": identifier,
        "repository": {
            "name": REPOSITORY_NAME,
            "base_commit": view.tagged_commit,
            "working_tree_used_as_input": False,
        },
        "package": {
            "name": package,
            "path": package_row.get("path"),
            "stage": pair.ticket.get("stage"),
            "phase": pair.ticket.get("phase"),
            "wave": pair.ticket.get("wave"),
        },
        "draft": {
            "path": pair.context_path,
            "git_blob_id": pair.context_blob,
            "exact_file_sha256": pair.context_sha256,
            "source_ceiling_class": pair.source_ceiling_class,
        },
        "artifact_candidate": {
            "relative_path": bundle_relative,
            "sha256": artifact_sha256,
            "bytes": len(bundle_bytes),
            "format": ARTIFACT_FORMAT,
            "local_file_is_immutable_artifact_ref": False,
        },
        "candidate_metadata_path": candidate_relative,
        "sources": sources,
        "registry_fragments": fragments,
        "accepted_handoffs": handoffs,
        "required_unavailable_checks": list(pair.unavailable_checks),
        "preflight_checks": checks,
        "reason_codes": [],
        "verification": {
            "source_count": len(sources),
            "registry_fragment_count": len(fragments),
            "accepted_handoff_count": len(handoffs),
            "forbidden_path_scan_passed": True,
            "bundle_roundtrip_verified": True,
            "local_output_readback_required": True,
            "authoritative_artifact_store_readback_verified": False,
        },
        "manifest_projection": manifest_projection,
        "ordinary_artifact_writes": [bundle_relative, candidate_relative],
        "control_record_mutations": [],
        "authority": authority_map(),
    }
    candidate["candidate_sha256"] = candidate_metadata_digest(candidate)
    if not assert_candidate_digest(candidate):
        raise CandidateFailure(
            "CONTEXT_ARTIFACT_BUILDER_CONTRACT_MISMATCH",
            "candidate metadata digest is not reproducible",
        )
    candidate_bytes = canonical_json_bytes(candidate)
    return CandidateBuild(
        candidate,
        candidate_bytes,
        bundle_bytes,
        candidate_relative,
        bundle_relative,
    )


def write_candidate(root: str | Path, build: CandidateBuild) -> tuple[Path, Path]:
    root_path = Path(root).resolve()
    bundle_path = root_path / build.bundle_relative_path
    candidate_path = root_path / build.candidate_relative_path
    write_exact_idempotent(root_path, bundle_path, build.bundle_bytes)
    write_exact_idempotent(root_path, candidate_path, build.candidate_bytes)
    if bundle_path.read_bytes() != build.bundle_bytes or candidate_path.read_bytes() != build.candidate_bytes:
        raise CandidateFailure(
            "CANDIDATE_OUTPUT_WRITE_FAILED", "candidate local readback failed"
        )
    return bundle_path, candidate_path
