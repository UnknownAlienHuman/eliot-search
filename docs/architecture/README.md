# ELIOT Search — architecture

ELIOT Search helps agents and people understand codebases and document collections without reading the whole
corpus. It finds useful evidence, shows its origin and makes the next source read or exact check obvious.
[Implementation](../implementation/README.md) owns delivery order and mechanisms; this page owns product invariants.
Historical Architecture 8.4 and task packets do not override it.

## Product

Users select local repositories/corpora and receive ranked source locations with short verified excerpts,
revision identity and explicit gaps. They can read exact evidence, orient in an unfamiliar project and compare
implementations. Search operates through its own CLI/local interface without a model, agent controller or ELIOT
integration. Large repositories and portfolios remain core workloads, not optional demos.

## Result contract

A citation binds the retained revision that supplied its bytes. Ranking proposes useful reading, not exhaustive
coverage, uniqueness, semantic equivalence or absence. Exact checks report their actual scanned inventory and
representation. Unsupported formats, missing revisions, stale observations and unfinished work stay visible.

Committed state survives restart; lost derived indexes rebuild. Updates converge to current selected content.
Preparation reuses unchanged work. Publication starts with complete fresh generations and gains source-local
mutation only where declared corpus budgets require it; do not promise that the reference profile avoids every
unrelated index rewrite. Interactive work remains bounded/cancellable and takes priority under resource pressure.

## Invariants

**One source of evidence.** Immutable source/prepared objects and their coordinates establish citations. Qdrant is
the only indexed retrieval engine; control storage holds compact application metadata, not another search corpus.

**One coherent view.** A request selects a non-widening scope from one published view. Evidence and retrieval must
refer to that same view. Incomplete writes never become a mixed visible index. Recovery resolves real unfinished
effects; ordinary reads and pure transforms need no durable receipts.

**Lifetime follows ownership.** Current/pending references and live query windows preserve required data. Cleanup
also accounts for unresolved external writers; the last reader disappearing is not enough. Publication owns
whole-generation index cleanup. Local-object collection is explicit stopped-root maintenance, not a hot-path GC
or universal lease/legal-hold/purge platform.

**Results are not capabilities.** Cards carry exact source/revision/location/excerpt. Paging and bounded expansion
use one short-lived same-session window derived from the pinned view. It is not a durable history or second bearer-
security system and may expire on disconnect/restart.

**Availability is factual.** A handler works only when its real dependencies are constructed. A name, registry,
digest, qualification receipt or advertisement does not create behavior or grant request authority.

**Local single-user product.** [Local scope](LOCAL_SCOPE.md) selects one OS account, plain storage, same-host
Qdrant and owned-child stdio. Application encryption, key lifecycles, restricted clients and remote/multi-user
services are not planned features. Source input remains no-execute; selection never silently widens.

**Complexity must serve a consumer.** Prefer direct calls and concrete values. A trait/package/worker earns its
place through a real dependency, effect or substantial algorithm, not a hypothetical future backend. Reuse maintained
parsing, matching, serialization, storage and transport; do not build a compiler or lifecycle operating system.

**Hashes have a purpose.** Content identity and integrity use real hashes. Requests, plans, reviews, configuration
and local state transitions do not need digests merely to appear trustworthy. Trusted local exchange needs no
additional cryptographic authority platform.

**Replacement preserves user data.** Internal pre-release APIs/formats have no compatibility promise. Remove old
implementations and their exclusive fixtures, but preserve original files and sole retained copies. Unsupported
roots fail before writes; do not silently migrate or destroy them.

**Bound work, not prose.** Stages/temporary pages/unreferenced objects are bounded discoverable debris, not permanent
operation history. Keep finite pending state only where a real effect can outlive the process. Source identity,
revision, current location and reusable transform input remain separate facts.

**Completion is observable.** Deliver connected behavior and remove its displaced route. File size, crate count,
closed issues, source presence and structural checks cannot prove correct results, recovery, relevance or scale.

## Product boundary

Search owns source acquisition, retained revisions, preparation, indexed retrieval, exact verification and compact
evidence. Agent scheduling, task governance, general memory, online research, hosted services, legal-record
management and universal migration are outside it. Optional code intelligence, formats, semantic ranking and
adapters extend the same source/query path only with real consumers.
