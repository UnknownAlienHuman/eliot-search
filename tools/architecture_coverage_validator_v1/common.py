from __future__ import annotations

import re
import tomllib
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
NONE = "NONE"
_RESERVED_SIGNATURE_WORDS = {"if", "for", "while", "match", "loop", "return"}


def load(path: str) -> dict[str, Any]:
    return tomllib.loads((ROOT / path).read_text(encoding="utf-8"))


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def rows(document: dict[str, Any], key: str, name_key: str) -> dict[str, dict[str, Any]]:
    value = document.get(key)
    if not isinstance(value, list):
        raise ValueError(f"{key} must be an array of tables")
    result: dict[str, dict[str, Any]] = {}
    for row in value:
        if not isinstance(row, dict) or not isinstance(row.get(name_key), str):
            raise ValueError(f"invalid {key} row")
        name = row[name_key]
        if name in result:
            raise ValueError(f"duplicate {key} row {name}")
        result[name] = row
    return result


def require(errors: list[str], condition: bool, message: str) -> None:
    if not condition:
        errors.append(message)


def section_between(text: str, start: str, end: str | None) -> str:
    begin = text.find(start)
    if begin < 0:
        raise ValueError(f"missing section heading: {start}")
    begin += len(start)
    if end is None:
        return text[begin:]
    finish = text.find(end, begin)
    if finish < 0:
        raise ValueError(f"missing section heading: {end}")
    return text[begin:finish]


def fenced_blocks(text: str, language: str | None = None) -> list[str]:
    pattern = r"```[^\n]*\n(.*?)```" if language is None else rf"```{re.escape(language)}\s*\n(.*?)```"
    return re.findall(pattern, text, flags=re.DOTALL)


def top_level_yaml_labels(text: str) -> set[str]:
    labels: set[str] = set()
    for block in fenced_blocks(text, "yaml"):
        for line in block.splitlines():
            match = re.match(r"^([A-Za-z][A-Za-z0-9_<>@,.-]*):(?:\s|$)", line)
            if match:
                labels.add(match.group(1))
    return labels


def normalize_type_name(name: str) -> str:
    return name.split("<", 1)[0]


def operation_names(text: str) -> set[str]:
    names: set[str] = set()
    names.update(re.findall(r"^#{2,3}\s+`([a-z][a-z0-9_]*)\b", text, flags=re.MULTILINE))
    names.update(re.findall(r"`([a-z][a-z0-9_]*)\([^`]*\)`", text))
    for block in fenced_blocks(text):
        names.update(
            re.findall(
                r"^(?:pub\s+)?(?:async\s+)?(?:fn\s+)?([a-z][a-z0-9_]*)\s*\(",
                block,
                flags=re.MULTILINE,
            )
        )
    return names - _RESERVED_SIGNATURE_WORDS


def validate_module_ref(errors: list[str], ref: Any, modules: dict[str, set[str]], owner: str) -> None:
    if not isinstance(ref, str) or ":" not in ref:
        errors.append(f"{owner}: invalid module ref {ref!r}")
        return
    package, module = ref.split(":", 1)
    if package not in modules:
        errors.append(f"{owner}: unknown package in module ref {ref}")
    elif module not in modules[package]:
        errors.append(f"{owner}: unknown module in ref {ref}")


def validate_owner_pair(
    errors: list[str],
    package: Any,
    module: Any,
    modules: dict[str, set[str]],
    owner: str,
    *,
    allow_none: bool = True,
) -> None:
    if allow_none and package == NONE and module == NONE:
        return
    if not isinstance(package, str) or not isinstance(module, str):
        errors.append(f"{owner}: owner package/module must be strings")
        return
    validate_module_ref(errors, f"{package}:{module}", modules, owner)


def exact_type_registry_symbols(type_registry: str) -> set[str]:
    symbols: set[str] = set()

    bounds = section_between(type_registry, "## Bounds and collections", "## Opaque and display wrappers")
    for block in fenced_blocks(bounds, "text"):
        for line in block.splitlines():
            match = re.match(r"^(Bounded[A-Za-z0-9_]+(?:<[^>]+>)?)", line.strip())
            if match:
                symbols.add(match.group(1))
    symbols.update(top_level_yaml_labels(bounds))

    opaque = section_between(type_registry, "## Opaque and display wrappers", "## Identity and reference registry")
    symbols.update(re.findall(r"^\| `([^`]+)` \|", opaque, flags=re.MULTILINE))

    identity = section_between(type_registry, "## Identity and reference registry", "## Baseline semantic registries")
    identity_blocks = [
        [line.strip() for line in block.splitlines() if line.strip()]
        for block in fenced_blocks(identity, "text")
    ]
    if len(identity_blocks) < 3:
        raise ValueError("TYPE_REGISTRY identity section must contain three text registries")
    for block in identity_blocks[:3]:
        for line in block:
            for token in [part.strip().rstrip(".") for part in line.split(",")]:
                if token:
                    symbols.add(token)
    symbols.update(top_level_yaml_labels(identity))

    semantic = section_between(type_registry, "## Baseline semantic registries", "## Coverage and freshness records")
    for block in fenced_blocks(semantic, "text"):
        symbols.update(re.findall(r"^([A-Z][A-Za-z0-9_]+)\s*=", block, flags=re.MULTILINE))
    if "`EntityKind`" in semantic:
        symbols.add("EntityKind")

    coverage = section_between(type_registry, "## Coverage and freshness records", "## Port-support records")
    symbols.update(top_level_yaml_labels(coverage))
    port_support = section_between(type_registry, "## Port-support records", "## Ownership and visibility summary")
    symbols.update(top_level_yaml_labels(port_support))
    return symbols
