#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GRAPH = ROOT / "tools/coverage_graph_v2.py"


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one occurrence, found {count}")
    return text.replace(old, new, 1)


def main() -> int:
    text = GRAPH.read_text(encoding="utf-8")
    marker = '''SUPPLEMENT_NAMES = {
    "W7_HARDENING.md",
    "P18_SCALE.md",
    "W8_INTEGRATION.md",
    "W10_INTEGRATION.md",
    "W8_CLIENT.md",
    "W10_OPTIONAL_EVALUATION.md",
}
'''
    addition = marker + '''GENERATED_MARKDOWN = {
    "docs/handoff/COVERAGE_GRAPH_V2.md",
    "docs/handoff/PACKAGE_MAP_INDEX_V2.md",
}
'''
    text = replace_once(text, marker, addition, "generated Markdown declaration")
    old = '''    selected_markdown = [path for path in git_files() if path.endswith(".md") and not path.startswith("artifacts/") and not path.startswith("docs/generated/")]
'''
    new = '''    selected_markdown = [
        path
        for path in git_files()
        if path.endswith(".md")
        and not path.startswith("artifacts/")
        and not path.startswith("docs/generated/")
        and path not in GENERATED_MARKDOWN
    ]
'''
    text = replace_once(text, old, new, "generated Markdown source exclusion")
    GRAPH.write_text(text, encoding="utf-8", newline="\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
