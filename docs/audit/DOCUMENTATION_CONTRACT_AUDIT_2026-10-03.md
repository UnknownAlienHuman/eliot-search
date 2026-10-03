# Documentation and product-contract audit — 2026-10-03

**Audited head:** `50b1e1b4f561e8f4edac86c4a9882c32fe613356`

## Verdict

The normative product direction is now correct:

- standalone Rust product;
- Qdrant as the only indexed/search database;
- source/preparation evidence outside Qdrant;
- redb as bounded technical control state;
- agent-oriented navigation, comparison and exact verification;
- no Search-local swarm controller.

Architecture Part I already contains most of the required analysis model: explicit source views,
reference portfolios, lexical/structural retrieval, deterministic fusion, subject resolution,
cross-repository comparison, provenance, coverage, exact proof and source-backed handles.

The documentation was still incomplete or contradictory in five material areas.

## Finding 1 — current source status was frequently false

Several package READMEs said `behavior is intentionally unimplemented` even though substantial Rust
kernels exist. Other documents described source presence without distinguishing daemon integration or
qualification. This creates two opposite errors:

- agents may rewrite existing kernels;
- agents may treat unintegrated/unqualified source as a finished capability.

A single implementation-status matrix is added in
[`docs/product/IMPLEMENTATION_STATUS.md`](../product/IMPLEMENTATION_STATUS.md). It uses separate
`CONTRACT`, `SOURCE`, `INTEGRATED`, `CHECKED`, `QUALIFIED` and `ENABLED` states. Critical package
entrypoints are corrected in this change; full repository reconciliation remains #220.

## Finding 2 — the architecture master mixes current and historical authority

The combined master contains normative Architecture Part I and an obsolete Part II `Codex Handoff 2.7`
that still says `Entry point: P00 only`. Root docs explain the precedence, but direct readers can still
follow the wrong program.

The safe permanent fix is a byte-identical extraction of Part I with preserved SHA-256 and an archived
handoff pointer. This is tracked by #220. Until then, the architecture README and root instructions are
the mandatory entry point.

## Finding 3 — the agent workflow had no complete public entry path

The architecture supports sophisticated retrieval after a caller identifies a symbol, path, exact
predicate or recipe. The closed v1 recipe set does not provide:

- a normal bounded free-text information need;
- a first-class scope-orientation response;
- a complete supported UX for registering a repository fleet and creating an immutable portfolio;
- a generic leaf adapter for common agent harnesses.

The work is split into:

- #213 — free-text retrieval and orientation contracts;
- #218 — corpus/repository-portfolio management;
- #219 — generic agent tool adapter;
- #221 — concrete baseline ranking/profile semantics.

This split prevents a client from filling the gap with raw Qdrant access or an unversioned ad-hoc prompt
protocol.

## Finding 4 — indexes and heuristics were described abstractly

The architecture correctly forbids another search database, but the product docs needed a clearer split:

### Physical indexed substrate

Qdrant owns sparse/optional dense vectors, payload indexes, filtered query execution, filtered IDF,
count/scroll and exact point readback.

### Eliot-owned projections and heuristics

Search owns unit selection, lexical features, structural facts, exact metadata predicates, scoring
population boundaries, query routing, deterministic rank fusion, evidence-role quotas, source/lineage
caps, fork/mirror collapse, currentness, candidate validation and exact follow-up suggestions.

The product guide is expanded accordingly. A concrete accepted baseline profile and qualification corpus
are still required under #221.

## Finding 5 — controller material remains operationally discoverable

ADR 0005 superseded ticket/lease/launch-state authority, but `xtask`, workflows, schemas and historical
documents still expose controller operations. A root warning alone is insufficient because agents may
find those files through search and start maintaining them.

Removal and de-registration remain #214. Historical information is recoverable through Git; it should not
remain an active required path.

## Confirmed non-gaps

The following design areas are already present in Architecture Part I and should not be reinvented:

- Qdrant-only production indexing;
- immutable source truth and exact readback;
- safe no-execute acquisition;
- explicit corpus and reference-portfolio identities;
- one projection membership per point;
- filtered retrieval/IDF noninterference;
- sparse lexical baseline and optional semantic profiles;
- subject ambiguity rather than silent selection;
- cross-repository comparison and lineage collapse;
- exact frozen-denominator verification;
- current-workspace observation gaps and overlays;
- rebuild after Qdrant loss;
- typed partial/degraded coverage;
- standalone and optional leaf-adapter boundaries.

## Correct implementation priority

Documentation must not redirect work from the product spine. The next agent should prioritize:

1. canonical Qdrant identity/payload/filter/publication integration;
2. daemon compilation and live Qdrant composition;
3. real end-to-end source-backed retrieval;
4. existing recipe delivery;
5. corpus/portfolio management and general agent retrieval;
6. scale, documents and optional semantic depth;
7. release qualification.

A new planning/control system is not a prerequisite for any of these steps.
