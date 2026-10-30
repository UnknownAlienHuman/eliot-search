<a id="search-qdrant-supervisor--optional-process-wrapper-not-runtime-authority"></a>
# Legacy Qdrant supervisor — not a baseline dependency

The product connects to an operator-started local server through the [bridge](../search-qdrant-bridge/FUNCTIONS.md).
It does not need another process manager to perform search. This directory still contains old source; it has
not been removed by the documentation changes.

<a id="product-order"></a>
<a id="completion"></a>
## Work route

[FUNCTIONS.md](FUNCTIONS.md) owns the caller/removal boundary. Connect the actual daemon to the bridge first;
keep server operation separate from CLI-owned child cleanup. Do not add a supervisor constructor to satisfy
old types or fixtures.

<a id="if-managed-child-mode-is-selected"></a>
<a id="removal-boundary"></a>
<a id="historical-baseline-inventory"></a>
## Deferred plans and history

Managed-server convenience requires a separate explicit selection after the baseline, not implementation of
this old framework. The [earlier inventory and proposed modes](https://github.com/UnknownAlienHuman/eliot-search/blob/7ed46f79f07a23f01f75f5e85a96248dc42e280b/crates/search-index-qdrant/search-qdrant-supervisor/README.md)
are historical evidence, not an active package/API specification.
