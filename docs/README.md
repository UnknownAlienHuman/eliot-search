# Documentation index

ELIOT Search is a local, source-backed search application. Target design is not implemented behavior.
Choose one route; do not load the whole documentation tree.

## Read by task

| Task | Read |
|---|---|
| Understand what to build | [Architecture](architecture/README.md), [local scope](architecture/LOCAL_SCOPE.md) |
| Continue implementation | [Resume](audit/ANALYSIS_REGISTRY.md#resume) → current [stage](implementation/README.md#delivery-sequence) |
| Find the actual files/callers | [Code map](implementation/CODE_MAP.md) |
| Check a change / update documentation | [Working guide](implementation/WORKING_GUIDE.md) |
| Change configuration | [Typed settings](../crates/search-config/FUNCTIONS.md) |
| Change root or file publication | [Root](../crates/search-runtime/search-runtime-owner/FUNCTIONS.md) |
| Change retained objects | [Object store](../crates/search-source/search-revision-store/FUNCTIONS.md) |
| Change source discovery/current state | [Source family](../crates/search-source/README.md) |
| Change preparation | [Materializer](../crates/search-prep/search-materializer/FUNCTIONS.md), [unitizer](../crates/search-prep/search-unitizer/FUNCTIONS.md) |
| Change retrieval/index | [Lexical profile](../crates/search-lexical/FUNCTIONS.md), [Qdrant bridge](../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) |
| Change publication/recovery/cleanup | [Publisher](../crates/search-index-qdrant/search-publication/FUNCTIONS.md) |
| Change query/session/output | [Query](../crates/search-query/README.md), [protocol](../crates/search-provider-protocol/FUNCTIONS.md), [CLI](../bins/eliot-search/FUNCTIONS.md) |
| Check an upstream assumption | Affected [donor evidence](audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md), then official API and actual caller |
| Plan later behavioral verification | [Test cutover](implementation/TEST_CUTOVER.md) |

## Authority and maintenance

Architecture/local scope own the product boundary. Implementation owns order and cross-owner meaning.
Each owner's FUNCTIONS.md owns its algorithm and failure behavior. AGENTS.md owns editing constraints;
READMEs route readers; the registry records current evidence and next work. Do not copy an algorithm into them.

[Update rules](implementation/WORKING_GUIDE.md#what-to-update) specify which files change with each kind of work.
A code map is an inspection aid, not a mandatory crate graph. Fix conflicting text at its owner and incoming
references in the same change. Keep useful section anchors or update callers of them.

## Published documentation versus code

Main's source still includes legacy implementations. #610's source removals and dependency changes apply only
to their named branch revisions. An absent caller in an old audit is not permission to delete current code.
Read actual manifests, dispatch and consumers before acting.

Old Architecture 8.4, P00, swarm, wave and qualification inputs are historical even when retained on disk for old
tooling. They do not restore encryption, grants, compatibility promises or acceptance gates. Do not read them
unless an actual deletion/caller investigation needs them.

## Supporting navigation

[Product workflows](product/AGENT_ANALYSIS_FRAMEWORK.md) · [Owner map](implementation/README.md#owners) ·
[Dependency/lifetime graph](handoff/DEPENDENCY_GRAPH.md) · [ADRs](adr/README.md) · [Audit](audit/README.md)
