# Documentation index

ELIOT Search is a standalone source-backed search application. The target architecture is not a claim that the
current binaries implement it. Start with the relevant row below; do not load every linked document.

## Published documentation versus code

These rules are published on `main` independently of PR #610's source refactor. Statements about inspected
callers, removed code or previous compiler runs apply to their named revisions; they are not proof that those
changes are present in `main`. Use the [registry](audit/ANALYSIS_REGISTRY.md#resume) and actual checked-out source.

The current authority path is this index → architecture/delivery sequence → affected owner. Retained old
Architecture 8.4, P00, wave, swarm and qualification packets outside that path are historical inputs to legacy
tooling, not additional instructions. They cannot override the current owner or require restoring retired APIs.

## Read by task

| Task | Short path |
|---|---|
| Understand the product | [Architecture](architecture/README.md) |
| Continue implementation | [Registry resume](audit/ANALYSIS_REGISTRY.md#resume) → current [delivery stage](implementation/README.md#delivery-sequence) → affected owner |
| Find the owner of a rule | [Owner map](implementation/README.md#owners) |
| Change configuration | [Typed settings](../crates/search-config/FUNCTIONS.md) |
| Change root initialization/platform publication | [Root recipe](../crates/search-runtime/search-runtime-owner/FUNCTIONS.md) |
| Change stored source/prepared bytes | [Object recipe](../crates/search-source/search-revision-store/FUNCTIONS.md) |
| Change lexical meaning or physical/query profiles | [Lexical profile](../crates/search-lexical/FUNCTIONS.md) |
| Change Qdrant calls | [Bridge recipe](../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) |
| Change publication, recovery or cleanup | [Publication recipe](../crates/search-index-qdrant/search-publication/FUNCTIONS.md) |
| Change results or CLI | [Query](../crates/search-query/README.md) or [CLI recipe](../bins/eliot-search/FUNCTIONS.md) |
| Change a dependency/platform assumption | Relevant [donor evidence](audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md), then its actual consumer |
| Remove obsolete tests | Relevant section of [test cutover](implementation/TEST_CUTOVER.md) |

## Authority and maintenance

| Document | Owns | Does not own |
|---|---|---|
| Architecture + product docs | Purpose, invariants and observable outcomes | Package layout, audit history |
| Implementation | Delivery order, cross-owner values and owner map | Copies of local algorithms |
| Owner `FUNCTIONS.md` | Concrete algorithm, failures, source seams and removal boundary | Another global architecture |
| `AGENTS.md` | Working method and local editing constraints | Duplicate algorithm/profile specifications |
| README/index | Purpose, status pointer and navigation | A competing rule set |
| Registry | Inspected revision, changes, checks and remaining work | Permission or runtime readiness |
| Donor register | Versioned upstream evidence and its limits | Installed capability or product qualification |

Resolve a conflict in the owning rule and update its references in the same change. Do not implement both versions.
Keep product invariants; a local recipe cannot silently weaken them. Issues track responsibility and acceptance
and link to the owner rather than reproducing the algorithm. Existing code shows what runs, not what must survive.

Keep important anchors stable. When moving a section, update incoming links or retain a meaningful anchor at its
replacement. Put historical rationale in Git-linked evidence, not on every implementation reading path.

## Supporting navigation

[Product workflows](product/AGENT_ANALYSIS_FRAMEWORK.md) · [Configuration](config/README.md) ·
[Dependency/lifetime graph](handoff/DEPENDENCY_GRAPH.md) · [ADRs](adr/README.md) ·
[Audit records](audit/README.md) · [Windows Qdrant](runtime/QDRANT_NATIVE_WINDOWS.md)

Historical material remains in Git history and, until its legacy consumers are removed, may also remain on disk.
Remove those files with the consuming tooling; their presence does not make them current requirements.
