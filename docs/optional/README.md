# Optional depth

No optional adapter/worker is required for baseline Search. The old Eliot adapter and model/document refusal
binaries were removed with their exclusive fixtures; their names never established an implemented capability.

| Real selected need | Small implementation boundary | Historical investigation only |
|---|---|---|
| Model/reranker | Exact model/task/input/options, bounded result; reuse preparation/query and preserve source evidence. Prefer in-process code unless a real isolation need justifies a child. | [Earlier model map](https://github.com/UnknownAlienHuman/eliot-search/blob/f3d710131aa3b06a63198694bed7ec82b1e251df/bins/eliot-search-model-worker/FUNCTIONS.md) |
| PDF/document extraction | Exact retained bytes → bounded mapped text/page coordinates and gaps → ordinary prepared object. No live-path reread or query-time extraction. | [Earlier document map](https://github.com/UnknownAlienHuman/eliot-search/blob/f3d710131aa3b06a63198694bed7ec82b1e251df/bins/eliot-search-doc-worker/FUNCTIONS.md) |
| MCP/ELIOT leaf | Maintained protocol SDK maps implemented operations to the same local client; no second query/window store, grants or backend access. | [Earlier adapter map](https://github.com/UnknownAlienHuman/eliot-search/blob/f3d710131aa3b06a63198694bed7ec82b1e251df/crates/search-eliot-adapter/FUNCTIONS.md) |

Read the [product workflows](../product/AGENT_ANALYSIS_FRAMEWORK.md#optional-depth) and current delivery stage.
Reverify exact upstream API/version/license and actual callers at implementation; old investigation is not an
installed dependency or reserved pin. Do not recreate placeholder packages, provider registries or refusal binaries.
