# Exact search over retained evidence

One request scans one explicit pinned inventory and named representation with one predicate. Ranked Qdrant hits
never establish exhaustive absence. [FUNCTIONS.md](FUNCTIONS.md) owns scanning, bounds, integrity and result meaning.

## Current source

[Prior source inventory](https://github.com/UnknownAlienHuman/eliot-search/blob/6ae24197a1be8695ede5bb7d12dfaf97f5bca1fc/crates/search-query/search-exact/README.md#current-source)
locates literal/proof/spine paths. Recheck callers before keeping the useful matcher and removing duplicate proof machinery.

## Target request

Use the [input-domain rules](FUNCTIONS.md#input-semantics-are-part-of-the-request) and
[execution recipe](FUNCTIONS.md#one-execution). Raw bytes, decoded text and extracted text are different requests.
Select local source IDs from the pinned view; no grants, restricted-user scope or second inventory.

## Completeness

Use [integrity and output rules](FUNCTIONS.md#integrity-memory-and-output). Complete no-match requires all selected
domain items verified/scanned without gaps. A presentation limit is distinct from aborted input scanning.
Optional paging uses the existing query window; it does not make unfinished work complete.

## Optional predicates

Single literal first. Add another predicate only with a real consumer and explicit domain/bounds; the
[regex rule](FUNCTIONS.md#regex-and-other-predicates) owns later engine semantics. No compile-plan, job or receipt service.
