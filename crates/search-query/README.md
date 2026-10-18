# Query and results

The [implementation guide](../../docs/implementation/README.md) connects typed request → scope/view →
retrieval → source validation → result. These are the existing owners, not mandatory future crates.

| Existing code | Useful role / current change |
|---|---|
| [access](search-access/) | Resolve real selected scope and any actual client restriction before retrieval. Remove mandatory grants for the owning local client. |
| [query planner](search-query-planner/) | Compile actual query/profile/limits. Its current compiler **consumes** an externally supplied task plan; remove the need to manufacture that plan and its proof graph. |
| [retrieval executor](search-retrieval-executor/) | Execute useful retrieval legs and combine contributions to the same evidence. Connect the real backend and propagate one deadline. |
| [candidate validator](search-candidate-validator/) | Resolve manifest/unit coordinates and verify the retained revision's actual bytes/range. Supplied digest equality is insufficient. |
| [result projector](search-result-projector/) | Format bounded source locations/excerpts, retaining the established result order. |
| [continuation](search-continuation/), [handles](search-handles/) | Consolidate with the useful daemon cursor/delivery code into one short-lived query window; remove duplicate catalogues and historical terminal records. |
| [exact](search-exact/) | Explicit bounded source scan over the stated scope, with accurate coverage. |
| [overlay](search-overlay/), [subject resolver](search-subject-resolver/), [comparator](search-comparator/) | Selected current-buffer, orientation and comparison behavior through the same query path. |

Source/revision/range is the baseline result reference. Add an opaque in-memory locator only for a
consumer that needs expansion. It can expire on restart; a last page must not prematurely destroy
items still needed by a live locator. Neither paging nor local references require another persistent
identity/authorization database.
