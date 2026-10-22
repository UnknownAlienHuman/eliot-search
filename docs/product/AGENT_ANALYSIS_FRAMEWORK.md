# Agent-oriented analysis behavior

ELIOT Search should help an agent or person understand a large repository or document collection with fewer source reads while keeping every claim tied to exact retained evidence.

This is a product-behavior document. It does not prescribe a recipe host, graph database, plan fingerprint, capability registry, model loop or package topology.

## Core result

A user selects explicit sources and asks a question. Search returns a bounded ordered set of cards containing:

- exact source, revision, representation and location;
- a short verified excerpt;
- why the result was nominated or grouped;
- freshness, unsupported state and material gaps;
- an optional same-session item locator for more context;
- suggested exact checks only as suggestions, not completed proof.

Ranked retrieval is navigation. It does not prove identity, correctness, consensus, completeness or absence. Exact retained-source operations own those claims over the inventory and representation they actually evaluate.

## Required user workflows

### Orient an unfamiliar repository

Return useful entry files, definitions, configuration, tests and documentation from actual prepared facts and source locations. Show missing fact producers and published-versus-current gaps. Do not generate an architectural verdict from filenames or lexical rank.

### Inspect an entity

Start with an explicit location or actual observed definitions/occurrences. Preserve ambiguity between same-named definitions. A mention, comment or string is not automatically a definition; syntax is not compiler-resolved call truth.

### Compare selected implementations

Compare explicit source-backed subjects over requested axes such as interface, validation, side effects, callers, tests and documentation. Show exact evidence and unknown axes. Equal names or scores do not prove equivalence, and repeated forks/copies are not independent confirmation without real lineage evidence.

### Navigate documents

Use the same retained-source/preparation/publication/evidence path for supported sections, pages, tables and references. Derived text carries its profile, coordinates and loss/gaps. Citation does not automatically establish support or contradiction. No hidden network or model fallback.

### Verify an exact predicate

Evaluate a stated raw-byte, decoded-text, extracted-representation, symbol or structural predicate over one pinned inventory. Report complete-with-match, complete-no-match, incomplete or invalid with item/byte accounting. A lexical top-k response never substitutes for this operation.

## Progressive use

The first useful response should not require loading the entire corpus or generating a complete map. Search may return a few evidence-backed starting points and let the client continue, expand or request exact checks. Small output does not imply cheap upstream work; population scans, parsing, source integrity and retained reads remain measured costs.

One `QueryWindow` may retain the final ordered result set and pinned view for paging or bounded context in the same live session. Restart/disconnect may expire it. A one-shot child must not return an unusable continuation token.

## Retrieval and evidence boundary

Qdrant is the only indexed backend. Native BM25 and optional selected branches nominate candidates under the explicit published scope. Exact punctuation/name identity remains independent of word-token BM25. The selected base population controls IDF; excluded data must not influence rank, but the cost of scoped statistics must be measured.

Before emission, a nomination resolves through the pinned published evidence row to the exact retained object/range. The source reader, not echoed payload digests or caller booleans, verifies the bytes. A correct excerpt proves its origin, not ranking quality or corpus completeness.

## Optional depth

Syntax outlines, SCIP/LSP facts, document extractors, editor overlays, models and MCP are optional concrete producers or adapters. Add one only with:

- a real user workflow and caller;
- exact snapshot/profile/input identity;
- bounded work and explicit unsupported cases;
- the same evidence-card/query-window path;
- deletion of the displaced scaffold or duplicate owner.

A model-generated label or summary is derived assistance and never replaces the source excerpt. Search does not own free-form research planning, web crawling, hidden reasoning, task authority or mutable notebook facts.

## Evidence for delivery

A connected implementation should demonstrate, on ordinary source examples:

- an unfamiliar repository with useful entry points;
- duplicate symbol names and mention-versus-definition ambiguity;
- an implementation with relevant tests/documentation;
- two selected implementations with one unsupported comparison axis;
- known copies that are not counted as independent evidence;
- a source head newer than the published view;
- exact raw/decoded behavior and truthful incomplete cases;
- bounded page/expansion behavior without rereading the whole result set.

Measure time to first useful evidence, source bytes/ranges read, candidate quality, false analogues, backend work, points rewritten per edit and context returned. Do not introduce an evaluation operating system merely to record those observations.
