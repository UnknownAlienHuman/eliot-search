from __future__ import annotations

import json
import tomllib
from typing import Any, Mapping, Sequence

from accepted_evidence_digest_v1 import EvidenceDigestError, accepted_evidence_digest
from context_artifact_builder_v1.bundle import BundleBlock, parse_bundle
from ticket_issuance_planner_v2.core import exact_sha256

from .core import (
    ARTIFACT_FORMAT,
    INSTANCE_STATUS,
    MaterializationPlanError,
    REPOSITORY,
    require_sha,
)


def q(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def b(value: bool) -> str:
    return "true" if value else "false"


def inline_table(value: Mapping[str, Any], order: Sequence[str]) -> str:
    parts: list[str] = []
    for key in order:
        item = value[key]
        if isinstance(item, str):
            rendered = q(item)
        elif isinstance(item, bool):
            rendered = b(item)
        elif isinstance(item, int) and not isinstance(item, bool):
            rendered = str(item)
        elif isinstance(item, dict):
            rendered = inline_table(item, tuple(item.keys()))
        else:
            raise MaterializationPlanError("MATERIALIZATION_RENDER_INVALID", f"unsupported inline value for {key}")
        parts.append(f"{key} = {rendered}")
    return "{ " + ", ".join(parts) + " }"


def accepted_handoff_projection(block: BundleBlock, base_commit: str) -> dict[str, Any]:
    try:
        record = tomllib.loads(block.content.decode("utf-8", "strict"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as exc:
        raise MaterializationPlanError("MATERIALIZATION_HANDOFF_INVALID", f"accepted handoff parse failed: {exc}") from exc
    if record.get("record_kind") != "package_handoff_v1" or record.get("status") != "ACCEPTED":
        raise MaterializationPlanError("MATERIALIZATION_HANDOFF_INVALID", "accepted handoff kind/status mismatch")
    identity = record.get("identity") if isinstance(record.get("identity"), dict) else {}
    accepted = record.get("accepted_code") if isinstance(record.get("accepted_code"), dict) else {}
    public = record.get("public_surface") if isinstance(record.get("public_surface"), dict) else {}
    compatibility = record.get("compatibility") if isinstance(record.get("compatibility"), dict) else {}
    package = identity.get("package")
    metadata = block.metadata
    if (
        package != metadata.get("package")
        or accepted.get("final_commit") != metadata.get("accepted_commit")
        or public.get("api_schema_digest") != metadata.get("api_schema_digest")
    ):
        raise MaterializationPlanError("MATERIALIZATION_HANDOFF_MISMATCH", "handoff block metadata differs from record")
    configuration = public.get("configuration_digest")
    if not isinstance(configuration, dict) or set(configuration) != {"state", "value"}:
        raise MaterializationPlanError("MATERIALIZATION_HANDOFF_INVALID", "configuration_digest is not OptionalV1")
    if configuration.get("state") == "ABSENT":
        if configuration.get("value") != "":
            raise MaterializationPlanError("MATERIALIZATION_HANDOFF_INVALID", "ABSENT configuration digest has a value")
    elif configuration.get("state") == "PRESEL":
        require_sha(configuration.get("value"), "configuration_digest.value")
    else:
        raise MaterializationPlanError("MATERIALIZATION_HANDOFF_INVALID", "configuration digest state invalid")
    evidence = record.get("evidence")
    try:
        evidence_digest = accepted_evidence_digest(evidence)
    except EvidenceDigestError as exc:
        raise MaterializationPlanError("MATERIALIZATION_HANDOFF_INVALID", str(exc)) from exc
    compat = compatibility.get("class")
    if compat not in {"COMPATIBLEE", "ADDITIVE", "BREAKING", "INTERNAL_ONLY"}:
        raise MaterializationPlanError("MATERIALIZATION_HANDOFF_INVALID", "compatibility class invalid")
    return {
        "package": package,
        "handoff_ref": {
            "repository": REPOSITORY,
            "commit": base_commit,
            "path": metadata.get("path"),
            "git_blob_id": metadata.get("git_blob_id"),
            "exact_record_file_sha256": metadata.get("exact_record_file_sha256"),
            "record_kind": "package_handoff_v1",
        },
        "accepted_commit": accepted.get("final_commit"),
        "api_schema_digest": public.get("api_schema_digest"),
        "configuration_digest": configuration,
        "evidence_digest": evidence_digest,
        "compatibility": compat,
    }


def derive_accepted_handoffs(bundle_bytes: bytes, candidate: Mapping[str, Any]) -> list[dict[str, Any]]:
    preamble, blocks = parse_bundle(bundle_bytes)
    if preamble.get("base_commit") != candidate["repository"]["base_commit"]:
        raise MaterializationPlanError("MATERIALIZATION_BUNDLE_MISMATCH", "bundle base commit differs from candidate")
    handoff_blocks = [block for block in blocks if block.kind == "accepted_handoff"]
    if len(handoff_blocks) != len(candidate.get("accepted_handoffs", [])):
        raise MaterializationPlanError("MATERIALIZATION_HANDOFF_MISMATCH", "accepted handoff count differs")
    result = [accepted_handoff_projection(block, preamble["base_commit"]) for block in handoff_blocks]
    if [item["package"] for item in result] != sorted(item["package"] for item in result):
        raise MaterializationPlanError("MATERIALIZATION_HANDOFF_MISMATCH", "accepted handoffs are not sorted by package")
    return result


def render_signed_payload(
    candidate: Mapping[str, Any],
    artifact_ref: Mapping[str, Any],
    context_id: str,
    operation_id: str,
    accepted_handoffs: Sequence[Mapping[str, Any]],
) -> bytes:
    package = candidate["package"]
    draft = candidate["draft"]
    lines = [
        "schema_version = 1",
        'record_kind = "context_manifest_v1"',
        f"status = {q(INSTANCE_STATUS)}",
        "",
        "[identity]",
        f"context_id = {q(context_id)}",
        f"operation_id = {q(operation_id)}",
        f"package = {q(package['name'])}",
        f"stage = {q(package['stage'])}",
        f"wave = {package['wave']}",
        f"base_commit = {q(candidate['repository']['base_commit'])}",
        "",
        "[draft]",
        f"path = {q(draft['path'])}",
        f"git_blob_id = {q(draft['git_blob_id'])}",
        f"exact_file_sha256 = {q(draft['exact_file_sha256'])}",
        "",
        "[artifact]",
        f"ref = {inline_table(artifact_ref, ('store_profile_ref', 'artifact_id', 'bytes', 'sha256'))}",
        f"sha256 = {q(artifact_ref['sha256'])}",
        f"bytes = {artifact_ref['bytes']}",
        f"format = {q(ARTIFACT_FORMAT)}",
    ]
    for source in candidate["sources"]:
        lines.extend([
            "",
            "[[sources]]",
            f"order = {source['order']}",
            f"repository_path = {q(source['repository_path'])}",
            f"git_blob_id = {q(source['git_blob_id'])}",
            f"exact_sha256 = {q(source['exact_sha256'])}",
            f"exact_bytes = {source['exact_bytes']}",
            f"materialization = {q(source['materialization'])}",
            f"materialized_sha256 = {q(source['materialized_sha256'])}",
            f"materialized_bytes = {source['materialized_bytes']}",
        ])
    for fragment in candidate["registry_fragments"]:
        lines.extend([
            "",
            "[[registry_fragments]]",
            f"order = {fragment['order']}",
            f"registry_path = {q(fragment['registry_path'])}",
            f"selector = {q(fragment['selector'])}",
            f"source_git_blob_id = {q(fragment['source_git_blob_id'])}",
            f"source_exact_sha256 = {q(fragment['source_exact_sha256'])}",
            f"selector_match_count = {fragment['selector_match_count']}",
            f"fragment_sha256 = {q(fragment['fragment_sha256'])}",
            f"fragment_bytes = {fragment['fragment_bytes']}",
        ])
    handoff_ref_order = ("repository", "commit", "path", "git_blob_id", "exact_record_file_sha256", "record_kind")
    for handoff in accepted_handoffs:
        lines.extend([
            "",
            "[[accepted_handoffs]]",
            f"package = {q(handoff['package'])}",
            f"handoff_ref = {inline_table(handoff['handoff_ref'], handoff_ref_order)}",
            f"accepted_commit = {q(handoff['accepted_commit'])}",
            f"api_schema_digest = {q(handoff['api_schema_digest'])}",
            f"configuration_digest = {inline_table(handoff['configuration_digest'], ('state', 'value'))}",
            f"evidence_digest = {q(handoff['evidence_digest'])}",
            f"compatibility = {q(handoY™l½µÁ…Ñ¥‰¥±¥Ñät¥ôˆ°(€€€€€€€t¤(€€€Ù•É¥™¥…Ñ¥½¸€ô…¹‘¥‘…Ñ•l‰Ù•É¥™¥…Ñ¥½¸‰t(€€€±¥¹•Ì¹•áÑ•¹¡l(€€€€€€€€ˆˆ°(€€€€€€€€‰mÙ•É¥™¥…Ñ¥½¹tˆ°(€€€€€€€˜‰Í½ÕÉ•}½Õ¹Ğ€ôíÙ•É¥™¥…Ñ¥½¹lÍ½ÕÉ•}½Õ¹Ğuôˆ°(€€€€€€€˜‰É•¥ÍÑÉå}™É…µ•¹Ñ}½Õ¹Ğ€ôíÙ•É¥™¥…Ñ¥½¹lÉ•¥ÍÑÉå}™É…µ•¹Ñ}½Õ¹Ğuôˆ°(€€€€€€€˜‰…•ÁÑ•‘}¡…¹‘½™™}½Õ¹Ğ€ôíÙ•É¥™¥…Ñ¥½¹l…•ÁÑ•‘}¡…¹‘½™™}½Õ¹Ğuôˆ°(€€€€€€€€‰É•…‘‰…­}Ù•É¥™¥•€ôÑÉÕ”ˆ°(€€€€€€€˜‰™½É‰¥‘‘•¹}Á…Ñ¡}Í…¹}Á…ÍÍ•€ôíˆ¡Ù•É¥™¥…Ñ¥½¹l™½É‰¥‘‘•¹}Á…Ñ¡}Í…¹}Á…ÍÍ•t¥ôˆ°(€€€€€€€€ˆˆ°(€€€t¤(€€€É•ÑÕÉ¸€ ‰q¸ˆ¹©½¥¸¡±¥¹•Ì¤€¬€‰q¸ˆ¤¹•¹½‘” ‰ÕÑ˜´àˆ¤(()‘•˜Í¥¹…ÑÕÉ•}¥¹±¥¹”¡Ù…±Õ”è5…ÁÁ¥¹mÍÑÈ°¹åt¤€´øÍÑÈè(€€€…ÉÑ¥™…Ñ}½É‘•È€ô€ ‰ÍÑ½É•}ÁÉ½™¥±•}É•˜ˆ°€‰…ÉÑ¥™…Ñ}¥ˆ°€‰‰åÑ•Ìˆ°€‰Í¡„ÈÔØˆ¤(€€€É•¹‘•É•€ôì(€€€€€€€€‰…ÁÁÉ½Ù…±}ÁÉ½™¥±•}É•˜ˆèÙ…±Õ•l‰…ÁÁÉ½Ù…±}ÁÉ½™¥±•}É•˜‰t°(€€€€€€€€‰…ÁÁÉ½Ù…±}…ÉÑ¥™…Ñ}É•˜ˆèÙ…±Õ•l‰…ÁÁÉ½Ù…±}…ÉÑ¥™…Ñ}É•˜‰t°(€€€€€€€€‰Í¥¹•‘}Á…å±½…‘}Í¡„ÈÔØˆèÙ…±Õ•l‰Í¥¹•‘}Á…å±½…‘}Í¡„ÈÔØ‰t°(€€€€€€€€‰…Ñ½É}¥‘•¹Ñ¥ÑäˆèÙ…±Õ•l‰…Ñ½É}¥‘•¹Ñ¥Ñä‰t°(€€€ô(€€€É•ÑÕÉ¸€‰ì€ˆ€¬€ˆ°€ˆ¹©½¥¸¡l(€€€€€€€˜‰…ÁÁÉ½Ù…±}ÁÉ½™¥±•}É•˜€ôíÄ¡É•¹‘•É•‘l…ÁÁÉ½Ù…±}ÁÉ½™¥±•}É•˜t¥ôˆ°(€€€€€€€˜‰…ÁÁÉ½Ù…±}…ÉÑ¥™…Ñ}É•˜€ôí¥¹±¥¹•}Ñ…‰±”¡É•¹‘•É•‘l…ÁÁÉ½Ù…±}…ÉÑ¥™…Ñ}É•˜t°…ÉÑ¥™…Ñ}½É‘•È¥ôˆ°(€€€€€€€˜‰Í¥¹•‘}Á…å±½…‘}Í¡„ÈÔØ€ôíÄ¡É•¹‘•É•‘lÍ¥¹•‘}Á…å±½…‘}Í¡„ÈÔØt¥ôˆ°(€€€€€€€˜‰…Ñ½É}¥‘•¹Ñ¥Ñä€ôíÄ¡É•¹‘•É•‘l…Ñ½É}¥‘•¹Ñ¥Ñät¥ôˆ°(€€€t¤€¬€ˆôˆ(()‘•˜É•¹‘•É}™Õ±±}µ…¹¥™•ÍĞ (€€€Á…å±½…è‰åÑ•Ì°(€€€É•…Ñ•‘}…ĞèÍÑÈ°(€€€µ…Ñ•É¥…±¥é•ÈèÍÑÈ°(€€€É•Ù¥•İ•ÈèÍÑÈ°(€€€Á…å±½…‘}Í¡„ÈÔØèÍÑÈ°(€€€µ…Ñ•É¥…±¥é•É}Í¥¹…ÑÕÉ”è5…ÁÁ¥¹mÍÑÈ°¹åt°(€€€É•Ù¥•İ•É}Í¥¹…ÑÕÉ”è5…ÁÁ¥¹mÍÑÈ°¹åt°(¤€´ø‰åÑ•Ìè(€€€¥˜¹½ĞÁ…å±½…¹•¹‘Íİ¥Ñ ¡ˆ‰q¹q¸ˆ¤è(€€€€€€€É…¥Í”5…Ñ•É¥…±¥é…Ñ¥½¹A±…¹ÉÉ½È ‰5QI%1%iQ%=9}I9I}%9Y1%ˆ°€‰Í¥¹•Á…å±½…µÕÍĞ•¹İ¥Ñ ½¹”‰±…¹¬Í•Á…É…Ñ½È±¥¹”ˆ¤(€€€Ñ•áĞ€ô€ (€€€€€€€€‰mÍ¥¹…ÑÕÉ•uq¸ˆ(€€€€€€€˜‰É•…Ñ•‘}…Ğ€ôíÄ¡É•…Ñ•‘}…Ğ¥õq¸ˆ(€€€€€€€˜‰µ…Ñ•É¥…±¥é•É}¥‘•¹Ñ¥Ñä€ôíÄ¡µ…Ñ•É¥…±¥é•È¥õq¸ˆ(€€€€€€€˜‰É•Ù¥•İ•É}¥‘•¹Ñ¥Ñä€ôíÄ¡É•Ù¥•İ•È¥õq¸ˆ(€€€€€€€˜‰É•½É‘}Í¡„ÈÔØ€ôíÄ¡Á…å±½…‘}Í¡„ÈÔØ¥õq¸ˆ(€€€€€€€˜‰µ…Ñ•É¥…±¥é•É}Í¥¹…ÑÕÉ•}É•˜€ôíÍ¥¹…ÑÕÉ•}¥¹±¥¹”¡µ…Ñ•É¥…±¥é•É}Í¥¹…ÑÕÉ”¥õq¸ˆ(€€€€€€€˜‰É•Ù¥•İ•É}Í¥¹…ÑÕÉ•}É•˜€ôíÍ¥¹…ÑÕÉ•}¥¹±¥¹”¡É•Ù¥•İ•É}Í¥¹…ÑÕÉ”¥õq¸ˆ(€€€€¤¹•¹½‘” ‰ÕÑ˜´àˆ¤(€€€É•ÑÕÉ¸Á…å±½…€¬Ñ•áĞ(