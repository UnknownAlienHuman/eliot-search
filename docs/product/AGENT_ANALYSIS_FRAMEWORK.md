# Agent analysis framework

ELIOT Search helps agents navigate and verify large repositories and document corpora without loading
all source into context. The [current architecture](../architecture/README.md) defines the local
baseline; [ADR 0006](../adr/0006-agent-analysis-framework-product-scope.md) preserves the broader mission.

## What the product should do

A query selects a repository/corpus, retrieves likely evidence and returns useful locations, exact
revision ranges, short previews and freshness/gaps. The client can reopen those bytes or request an
exact check. Qdrant finds indexed candidates; Search owns source snapshots, preparation, publication
and source validation. redb is small control state, not another search engine.

Ranked retrieval and source accuracy are different properties. Every cited snippet must match its
revision; a bounded top-k response still does not prove corpus-wide completeness or absence. Do not
require a full count/proof operation before returning useful navigation.

## User workflows

| User need | Useful result |
|---|---|
| Understand an unfamiliar repository | Likely entry files, definitions, configuration, tests and documentation; short reasons and exact places to read. |
| Find where to grep | A small set of relevant source ranges and suggested exact phrases/regex/symbol checks. |
| Inspect an entity | Definitions/references/callers and related tests/docs, with ambiguity shown rather than guessed away. |
| Compare implementations | Relevant implementations grouped by independent repository lineage; differences backed by exact source. |
| Read research/document corpora | Sections/pages/tables/references with native coordinates and source/profile identity. |
| Verify absence/completeness | A separately requested frozen-inventory scan with complete-match, complete-no-match, incomplete or invalidated outcome. |

The first two should become useful through the same indexed path before optional comparison, parser,
backup, encryption or delegated-client frameworks become prerequisites. Keep exact DIRECT scanning
available. Models may improve optional ranking but do not supply source truth or the baseline runtime.

## What exists and what is missing

The current default daemon/client serves DIRECT retained-source search. Real Qdrant transport, one
blocking Tokio facade, preparation, point/planner code, source validation and result owners exist, but
are not connected into a normal indexed user query.

The #213 schemas already exist:
[request/result types](../../crates/search-contracts/src/evidence.rs) and
[provider encoding](../../crates/search-provider-protocol/src/frame/client/evidence_schema.rs) for
`retrieve_evidence@1` and `orient_scope@1`. Simplify their mandatory plan/fence/trace metadata and exact-scan
budget assumptions, then connect their real serving path. Change the existing V1 host and V2 consumers
together; do not create another contract family, runtime or recipe registry.

## Result design

Start with the fields that let the user act: resolved scope/revision, relative location, exact range,
bounded snippet, why it matched, and any stale/missing/unsupported/truncated condition. Add optional
relations, lineage, evidence roles and suggested next reads as real producers exist. Avoid mandatory
empty metadata fields and opaque receipts that force agents to make more calls to understand a hit.

Orientation uses a cheap source guide from paths, headings, definitions and prepared coordinates.
Return selected snippets and a few useful next reads, adding nearby or linked locations only when
they help the question. No mandatory generated summary hierarchy or entity graph is needed. A model
summary is derived assistance; the source excerpt and its locator remain independently readable.

Source paths are locators; they do not replace immutable identity. Scores are hints, not confidence or
proof. Cross-population scores are not directly comparable. Forks/mirrors are not independent evidence.
Apply the resolved eligibility to retrieval and the explicit Qdrant IDF corpus; ordinary retrieval
filtering alone does not scope statistics. IDF counts physical vector-bearing points, not distinct files
or result groups.
Do not add excluded data back through ranking, groups or suggestions, or duplicate index points just
because a repository appears in multiple portfolios.

A continuation may retain a bounded candidate window and snapshot pin; expiry/restart may require a new
query. A normal read does not need a durable job, exported handle or provider ownership transfer.
Source restrictions take effect before their acknowledgement and invalidate affected outstanding reads.

## Extend the useful path

Plain text/source and local Qdrant BM25 come first. Native BM25 needs no model-worker service.
Use a syntax outline for names/signatures; import actual SCIP/LSP occurrences when their source snapshot
is known. An outline is not a semantic call graph. Add selected document profiles through the same
units/maps, projection and validation path. Markdown/JATS adapters and lexical
libraries already exist. Exact coordinate/loss maps matter when a parser transforms source. Document
macros, indexed code, hooks and remote resources are never executed by ordinary search.

Measure whether an agent finds the right implementation with fewer source reads and less context;
also measure recall, false analogues, first useful result, actual bytes/relations read, model calls and
points rewritten per edit. A small returned context can conceal expensive upstream work. See the
[implementation guide](../implementation/README.md) for concrete changes and selected extensions.
