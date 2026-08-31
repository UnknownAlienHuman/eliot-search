from __future__ import annotations

import json
from typing import Any, Mapping, Sequence

from ticket_issuance_planner_v2.core import canonical_json_bytes, exact_sha256

from .core import (
    ARTIFACT_FORMAT,
    BUNDLE_END,
    BUNDLE_MAGIC,
    MAX_BUNDLE_BYTES,
    BundleBlock,
    CandidateFailure,
)


def expected_header(kind: str, metadata: Mapping[str, Any]) -> str:
    if kind == "source":
        return f"--- repository-path: {metadata.get('repository_path', '')} ---"
    if kind == "registry_fragment":
        return (
            "--- registry-selector: "
            f"{metadata.get('registry_path', '')}::{metadata.get('selector', '')} ---"
        )
    if kind == "accepted_handoff":
        return f"--- accepted-handoff: {metadata.get('package', '')} ---"
    raise CandidateFailure("BUNDLE_FORMAT_INVALID", f"unknown bundle block kind: {kind}")


def render_bundle(
    preamble: Mapping[str, Any], blocks: Sequence[BundleBlock]
) -> bytes:
    output = bytearray(BUNDLE_MAGIC)
    output.extend(canonical_json_bytes(preamble))
    for block in blocks:
        metadata = dict(block.metadata)
        metadata["block_kind"] = block.kind
        metadata["content_bytes"] = len(block.content)
        metadata["content_sha256"] = exact_sha256(block.content)
        header = expected_header(block.kind, metadata)
        if block.header != header:
            raise CandidateFailure(
                "BUNDLE_FORMAT_INVALID",
                f"bundle header differs from canonical header: {block.header}",
            )
        output.extend(header.encode("utf-8"))
        output.extend(b"\n")
        output.extend(canonical_json_bytes(metadata))
        output.extend(block.content)
        output.extend(b"\n")
    output.extend(BUNDLE_END)
    if len(output) > MAX_BUNDLE_BYTES:
        raise CandidateFailure(
            "BUNDLE_SIZE_EXCEEDED",
            f"context artifact candidate exceeds {MAX_BUNDLE_BYTES} bytes",
        )
    return bytes(output)


def _read_line(data: bytes, offset: int) -> tuple[bytes, int]:
    end = data.find(b"\n", offset)
    if end < 0:
        raise CandidateFailure("BUNDLE_FORMAT_INVALID", "unterminated bundle line")
    return data[offset:end], end + 1


def _parse_canonical_json_line(raw: bytes, label: str) -> Mapping[str, Any]:
    try:
        value = json.loads(raw.decode("utf-8", "strict"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise CandidateFailure(
            "BUNDLE_FORMAT_INVALID", f"invalid {label} JSON"
        ) from exc
    if not isinstance(value, dict):
        raise CandidateFailure(
            "BUNDLE_FORMAT_INVALID", f"{label} JSON must be an object"
        )
    if canonical_json_bytes(value) != raw + b"\n":
        raise CandidateFailure(
            "BUNDLE_FORMAT_INVALID", f"{label} JSON is not canonical"
        )
    return value


def parse_bundle(data: bytes) -> tuple[Mapping[str, Any], list[BundleBlock]]:
    if len(data) > MAX_BUNDLE_BYTES or not data.startswith(BUNDLE_MAGIC):
        raise CandidateFailure("BUNDLE_FORMAT_INVALID", "invalid bundle magic or size")
    offset = len(BUNDLE_MAGIC)
    preamble_line, offset = _read_line(data, offset)
    preamble = _parse_canonical_json_line(preamble_line, "preamble")
    if preamble.get("artifact_format") != ARTIFACT_FORMAT:
        raise CandidateFailure("BUNDLE_FORMAT_INVALID", "artifact format mismatch")

    blocks: list[BundleBlock] = []
    while True:
        line, offset = _read_line(data, offset)
        if line + b"\n" == BUNDLE_END:
            break
        try:
            header = line.decode("utf-8", "strict")
        except UnicodeDecodeError as exc:
            raise CandidateFailure("BUNDLE_FORMAT_INVALID", "non-UTF-8 block header") from exc
        metadata_line, offset = _read_line(data, offset)
        metadata = _parse_canonical_json_line(metadata_line, "block metadata")
        kind = metadata.get("block_kind")
        size = metadata.get("content_bytes")
        digest = metadata.get("content_sha256")
        if not isinstance(kind, str) or not isinstance(size, int) or size < 0:
            raise CandidateFailure("BUNDLE_FORMAT_INVALID", "invalid block framing metadata")
        if offset + size >= len(data):
            raise CandidateFailure("BUNDLE_FORMAT_INVALID", "truncated block content")
        content = data[offset : offset + size]
        offset += size
        if data[offset : offset + 1] != b"\n":
            raise CandidateFailure("BUNDLE_FORMAT_INVALID", "missing block delimiter")
        offset += 1
        if digest != exact_sha256(content):
            raise CandidateFailure("BUNDLE_FORMAT_INVALID", "block digest mismatch")
        if header != expected_header(kind, metadata):
            raise CandidateFailure("BUNDLE_FORMAT_INVALID", "block header mismatch")
        clean_metadata = dict(metadata)
        clean_metadata.pop("block_kind", None)
        clean_metadata.pop("content_bytes", None)
        clean_metadata.pop("content_sha256", None)
        blocks.append(BundleBlock(kind, header, clean_metadata, content))
    if offset != len(data):
        raise CandidateFailure("BUNDLE_FORMAT_INVALID", "trailing bundle bytes")

    counts = (
        preamble.get("source_count"),
        preamble.get("registry_fragment_count"),
        preamble.get("accepted_handoff_count"),
    )
    if any(not isinstance(value, int) or value < 0 for value in counts):
        raise CandidateFailure("BUNDLE_FORMAT_INVALID", "invalid bundle counts")
    expected_total = sum(counts)
    if expected_total != len(blocks):
        raise CandidateFailure("BUNDLE_FORMAT_INVALID", "bundle count mismatch")
    return preamble, blocks
