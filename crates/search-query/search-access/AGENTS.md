# Working on search-access

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and [source map](FUNCTIONS.md).
They govern the local product and replace historical grant/proof contracts and package quotas.

Resolve the user's selected sources against application state and one pinned published view.
Build one eligibility population for compatible memberships. Share its base scope with candidate
retrieval and explicit Qdrant `params.idf.corpus`; query-only topical filters may further narrow hits.
Keep real exclusions before retrieval and statistics. Native BM25/IDF belongs to Qdrant.

Replace proof-dependent singleton legs with their real consumers: daemon admission, indexed executor,
pin handling and bridge filter input. The current executor rejects grouped multi-plan legs, so changing
the compiler alone is incomplete. Split requests only for actual different routes or incompatible
search settings; use a small bounded merge when those results must be combined.

The owning-account path uses selected local scope directly. Remove mandatory signed grants, overlap
certificates, predicate digests and accepted-record construction from that route. Restricted external
clients may need access enforcement at their real entrypoint; preserve effective restrictions for
those consumers without making their protocol the local query backbone.

Keep scope resolution read-only. Report unknown/empty scope, unavailable revisions and cancellation
clearly. Bound actual request work and avoid duplicate ingestion of the intended unit population;
membership count alone must not force one request per file. Final grouping does not change native IDF.

Change shared types and actual callers together, coordinating files with their active writer.
A contract-change issue, compatibility layer or new wrapper is not a prerequisite for an authorized
integration. Verify multi-membership execution, excluded-corpus behavior and pinned source reads.
Remove obsolete singleton/proof/grant fixtures with the replaced path.
