# Source map and implementation — search-access

The [current implementation](../../../docs/implementation/README.md) uses resolved local scope and
one pinned published view. Existing grant/proof types below describe source to simplify, not a
mandatory ceremony for each search.

## Actual operations and callers

| Source/API | Current behavior and change |
|---|---|
| [`intersect_scope`](src/lib.rs) | Intersects explicit memberships with known active registry bindings. Retain non-widening resolution and useful unknown/empty/stale results. The owning-account path supplies selected local scope directly. |
| [`compile_base_eligibility`](src/lib.rs) | Builds one membership plan and hashes its binding/route/generations. Replace repeated per-member plan digests with a concrete selected-population predicate tied to the actual published view. |
| [`compile_safe_legs`](src/lib.rs) | Without an overlap proof, emits one leg per membership and fails when membership count exceeds `max_legs`. A matching proof creates a leg containing multiple plans. Replace this split/proof dependency for compatible local scopes. |
| [`compile_pre_retrieval`](src/lib.rs) | Sequences grant, policy, scope, legs and live checks. [Daemon admission](../../../bins/eliot-searchd/src/access_composition/gate.rs) calls it; [standalone admission](../../../bins/eliot-searchd/src/access_composition/grant_authority/admission.rs) separately calls `intersect_scope` and `compile_safe_legs`. Change both real callers with the compiler. |
| [`validate_grant`, `recheck_live_access`, contamination helpers](src/lib.rs) | Separate actual restricted-client needs from local scope selection. Remove signed grant/template/fingerprint prerequisites from the owning-account route; do not fabricate accepted records. |
| [`SecurityMutationBarrier::apply_security_mutation` / `recover_security_mutation`](src/barrier.rs) | Existing restriction-effect path. Where a real consumer can revoke access during work, preserve its effective restriction and unfinished durable effects; do not make this optional subsystem the local query backbone. |

The [indexed executor](../search-retrieval-executor/src/indexed.rs) currently refuses
`eligibility_plans.len() != 1`, including the compiler's grouped result. Changing the compiler alone
does not connect union-scope retrieval. Adapt that consumer, pin handling and the bridge filter input
together. This is source analysis; no Rust integration fix is included in these documents.

## One compatible indexed population

Resolve the selected memberships once. Use one scope predicate and one published route/epoch; share
that base population between candidate retrieval and explicit `params.idf.corpus`. Extra topical
filters may restrict returned candidates without redefining the corpus. Keep real exclusions before
retrieval and IDF. Do not restore a global corpus when the selected one is empty.

The [bridge filter](../../search-index-qdrant/search-qdrant-bridge/src/real/filter.rs) already renders
a membership union; [`query_filtered`](../../search-index-qdrant/search-qdrant-bridge/src/real/queries/search.rs)
already calls Qdrant's explicit IDF corpus API. Native IDF is shard-local and counts physical points
carrying the sparse vector; see the [bridge limits](../../search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md#native-query-and-scope).
No runtime overlap certificate or separate application DF database is needed for the chosen local
unit population. Avoid duplicate ingestion; group returned hits only for presentation/diversity.

Use multiple requests only for genuinely different physical routes or incompatible retrieval settings.
Keep a small bounded merge when those results must be combined. A budget should bound actual query
work; membership count alone must not force thousands of requests or deny an ordinary repository.
Scope changes, cancellation and unavailable source revisions remain explicit results.

Verify that excluded data affects neither candidates nor selected-corpus statistics, multi-membership
scope reaches the real executor, and source reads use the pinned revision. Keep those behavior checks
while retiring proof-digest equality, grant ceremony and singleton-leg fixtures for the replaced path.
