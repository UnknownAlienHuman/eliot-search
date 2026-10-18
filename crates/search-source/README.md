# Sources

Selected roots become retained revisions and current source records. The
[implementation guide](../../docs/implementation/README.md) defines the connected ingestion result;
this is a map of existing code, not a requirement to retain every package.

| Existing code | Useful responsibility / simplification |
|---|---|
| [source registry](search-source-registry/), [source records](search-source-records/), [source identity](search-source-identity/) | Selected roots, source identity/locator and current revision. Consolidate repeated source/view models and lookups around these records. |
| [source reconcile](search-source-reconcile/) | Turn notifications and bounded inventories into work for changed, moved or missing sources. |
| [safe reader](search-safe-reader/) | Acquire stable bytes from the selected source without executing its contents. Pass those same bytes onward. |
| [revision store](search-revision-store/) | Existing immutable object mechanics and revision models. Its in-memory store is not the missing persistent application backend. |
| [source history](search-source-history/), [provenance](search-source-provenance/) | Keep relationships actually needed to reopen, trace or compare revisions; fold duplicate current/history bookkeeping into the source owner. |
| [admission](search-source-admission/), [receipt issuer](search-source-admission-receipt/) | Keep selected-source policy evaluation. Retire permanent proof/receipt handoffs that duplicate an in-process decision. |

Source bytes and content-revision identity must agree. Store the immutable object before committing its
head/reference; preparation can retry from that revision. An unsupported old format is rejected before
writes and left intact. There is no legacy-reader or migration prerequisite for the new application.
