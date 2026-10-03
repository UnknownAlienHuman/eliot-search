# ADR 0006 — Agent-oriented analysis and retrieval framework

- **Status:** accepted
- **Date:** 2026-10-03
- **Scope:** product mission, Qdrant boundary, agent-facing analysis workflow and large-corpus behavior
- **Architecture:** ELIOT Search 8.4
- **Extends:** ADR 0005 without changing the closed v1 wire recipes

## Context

ELIOT Search is sometimes described too narrowly as local source preparation and retrieval. That wording
can suggest a thin file finder or a replacement for `grep`. The intended product is broader: a standalone
analysis framework that lets agents navigate and verify very large repositories, portfolios of
repositories, research collections and mixed text corpora with minimal context consumption.

Qdrant already provides the physical indexed retrieval substrate. Rebuilding its vector/sparse storage,
payload filtering, count/scroll or query execution would be wasteful and would create a second search
engine. The product value of ELIOT Search lies in the semantics and evidence pipeline around Qdrant.

Architecture Part I already specifies most of that pipeline: immutable source truth, materialization,
lexical/structural projections, explicit portfolios, safe access-scoped legs, deterministic fusion,
subject resolution, comparison, exact verification, provenance, coverage, handles and source readback.
The active README and agent instructions must present those capabilities as the core product rather than
as optional details.

Two public-contract gaps remain visible:

1. the closed eleven-recipe v1 surface has no first-class bounded free-text retrieval request;
2. there is no first-class scope-orientation result that tells an agent what to read and which exact
   checks to execute next.

The product also lacks qualification at the repository-fleet/research-corpus scale implied by its mission.

## Decision

1. **ELIOT Search is an analysis framework, not only a locator.** It provides source-backed navigation,
   retrieval, inspection, comparison, provenance, coverage and exact verification across large explicit
   scopes.
2. **Qdrant is the sole physical indexed backend.** Search uses Qdrant's sparse/optional dense vectors,
   payload indexes, filtered retrieval, filtered IDF, count/scroll and exact point readback. Search does
   not add another production postings/vector/search database.
3. **Search owns the evidence pipeline around Qdrant.** Source identity, immutable revisions,
   materialization, unitization, coordinate maps, projection manifests, publication, currentness,
   access, planning, fusion, validation and result projection remain Eliot-owned responsibilities.
4. **Repository and document portfolios are first-class.** A query may target one active workspace, one
   corpus, explicit memberships or an immutable `ReferencePortfolioRevision` containing many
   repositories/documents. Large portfolios are partitioned into bounded safe legs and deterministically
   fused.
5. **The normal agent workflow is progressive.** Search should orient the scope, retrieve likely evidence,
   inspect/compare relevant entities, expose recommended reading, then compile exact checks where the
   claim matters. Clients should not need to read an entire corpus before taking the first grounded step.
6. **Heuristics propose; exact evidence decides.** Keyword, structural, sparse lexical, optional dense,
   rerank, boosts, diversity and lineage heuristics may prioritize candidates. They never prove identity,
   absence, correctness or consensus. Exact source readback and explicit exact scans own those claims.
7. **Agent-facing results are action-oriented but non-authoritative.** Results may include recommended
   reading, likely entry points, evidence-role groups and bounded suggested grep/regex/symbol/structural
   predicates. Suggestions are navigation hints until executed by the exact plane. Search does not issue
   tasks, approve changes or declare completion.
8. **Free-text retrieval and scope orientation require a versioned contract extension.** The existing
   eleven v1 recipes remain stable. Implementations must not smuggle heuristic free-text semantics into
   `find_text@1`, overload `locate@1`, or expose raw Qdrant queries. The extension is tracked by #213.
9. **Scale is a product property.** Warm latency alone is insufficient. Qualification must measure large
   monorepos and multi-repository portfolios, incremental updates, quality/recall, false analogues,
   time-to-first-useful result, resource use, restart and rebuild. This is tracked by #215.
10. **Research/document analysis uses the same pipeline.** Plain text/source/Git remain the first
    baseline. Qualified Markdown/HTML/LaTeX/PDF/Office/OCR profiles must preserve exact source identity,
    coordinate/loss maps, assurance and rebuild semantics. This is tracked by #216.
11. **No model is required in the hot path.** Deterministic lexical and structural retrieval is the
    baseline. Optional embeddings/rerank may improve recall only through explicit qualified profiles. A
    consuming agent/model interprets Search results; it cannot replace Search evidence controls.
12. **Controller boundaries remain unchanged.** Agent orchestration, Tasks, WorkScopes, GM/Governor,
    scheduling, mailboxes, reviews and finish authority remain outside Search under ADR 0005.

## Product workflow

```text
admit exact sources
→ retain immutable revisions
→ materialize and unitize with coordinate maps
→ create lexical/structural/optional semantic projections
→ publish a verified Qdrant collection epoch
→ compile one coherent access/currentness-scoped plan
→ retrieve candidates through bounded Qdrant legs
→ fuse/diversify/collapse lineages deterministically
→ reopen exact source revisions and validate candidates
→ return compact navigation, coverage and source handles
→ compile/execute exact verification where required
```

Index loss is recoverable because Qdrant is a projection, not source truth. Query admission remains
read-only. A top-k result never becomes a complete corpus statement.

## Required agent experience

For an unfamiliar repository or portfolio, the product should be able to return:

- a bounded scope summary and readiness/freshness state;
- likely entry files, sections, symbols and configuration points;
- definitions, callers, tests, documentation and relevant relations;
- comparable implementations grouped by independent repository lineage;
- exact source-backed handles for recommended reading;
- explicit ambiguity, unsupported modalities and coverage gaps;
- suggested exact phrase/regex/symbol/structural checks;
- provenance and corpus-delta information;
- progressive continuation without forcing the client to ingest the whole result set.

For research/document corpora the same result model applies to sections, pages, tables, references and
other qualified native coordinates.

## Compatibility

This ADR clarifies product mission and prioritization. It does not mutate existing serialized v1 recipes,
reason codes, collection schema or provider envelopes. New request/result shapes require the normal
versioned contract process and incompatible projection changes require a new collection generation.

Part I of the implementation master remains normative. The embedded Part II Codex handoff is historical
scaffolding and cannot narrow this product scope or reinstate ticket/lease implementation authorization.

## Consequences

- Qdrant integration remains the immediate critical path, not generic repository orchestration tooling.
- Query and result contracts must serve ordinary agent questions, not only callers that already know a
  symbol/path.
- Product evaluation must compare time-to-grounded-action and context/source reads against raw grep and
  current tools, not only report CRUD or latency.
- Document providers and optional semantic models stay replaceable and independently qualified.
- Residual controller-oriented tooling remains removal work under #214.

## Rejected alternatives

- **Expose Qdrant directly to agents:** leaks vendor types, bypasses access/currentness/source validation
  and makes clients reproduce Search semantics.
- **Build a second local index for exact/lexical convenience:** violates the single-index boundary and
  complicates publication/rebuild/currentness.
- **Make an LLM interpret every query inside Search:** adds latency, nondeterminism, cost and another
  authority boundary; models remain optional consumers/providers.
- **Treat grep as obsolete:** exact grep/regex/symbol/structural scans remain essential verification tools;
  Search should identify where and why to run them.
- **Return generated repository summaries as truth:** orientation must remain source-backed, bounded and
  explicit about coverage and uncertainty.
