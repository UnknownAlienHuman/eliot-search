# Agent analysis framework

ELIOT Search is a standalone Qdrant-backed framework for finding, organizing, comparing and exactly
verifying evidence in large local corpora. Its main consumers are coding/research agents and human
operators who need a grounded map of an unfamiliar repository or document collection before spending
large context windows on raw reads.

This guide explains product behavior. Architecture Part I and accepted product contracts remain
authoritative. Current implementation truth is tracked separately in
[`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md).

## 1. Product model

```text
immutable source truth and current workspace observations
                         ↓
materializations, units, structure and lexical features
                         ↓
verified projections published to Qdrant
                         ↓
access/currentness-scoped candidate retrieval
                         ↓
exact source readback, validation, provenance and coverage
                         ↓
compact agent-oriented navigation and optional exact verification
```

Qdrant is the physical index and candidate engine. ELIOT Search is the evidence and analysis layer around
it. redb stores technical control state; immutable source/preparation artifacts remain outside both.

## 2. Responsibility split

### 2.1 Qdrant owns the physical indexed substrate

Use Qdrant for:

- sparse and optional qualified dense/multivector storage;
- payload indexes and strict-mode filter enforcement;
- filtered query execution and filtered IDF populations;
- collection-local scoring;
- bounded count and scroll operations;
- exact point readback;
- acknowledged point/collection mutations.

Search does not reproduce those capabilities in Tantivy, Lucene, SQLite FTS, a local postings database or
a second vector store. It also does not expose Qdrant collection names, filters, point IDs or SDK types to
agents.

### 2.2 Search owns the evidence pipeline around Qdrant

Search adds:

- safe root/source admission and no-execute reading;
- stable source/repository/worktree/lineage identity;
- immutable retained revisions;
- materialization with coordinate and loss maps;
- deterministic unitization;
- lexical and structural features;
- projection manifests and collision-safe point identities;
- publication epochs, route cutover and rebuild;
- coherent workspace/corpus/portfolio views;
- access/currentness compilation before retrieval and IDF;
- bounded query planning and deterministic fusion;
- exact source readback and candidate validation;
- provenance, coverage, handles and exact verification.

## 3. Projection and index design

The product uses one physical Qdrant collection generation at a time, but that collection may contain
multiple Eliot-owned projection families.

### 3.1 Unit families

Prepared source revisions may produce bounded units such as:

- file or whole-document units;
- sections, headings and paragraphs;
- symbols, definitions and methods;
- references, callers and imports;
- tests and documentation;
- configuration items and predicates;
- tables, captions, references and qualified document regions.

A unit is tied to one exact representation and native coordinate map. It is not assumed stable across an
arbitrary reparse.

### 3.2 Named retrieval profiles

The baseline product needs independently versioned profiles for:

- code-oriented sparse lexical retrieval;
- language-neutral text/document sparse retrieval;
- exact keyword/facet predicates over permitted opaque metadata;
- structural fact retrieval;
- optional dense and rerank profiles after qualification.

A tokenizer, term mapping, weighting, vector dimension, collision policy or interpretation change creates
a new profile identity and normally a new collection generation. Search never silently switches providers
or profiles at runtime.

### 3.3 Payload boundary

Qdrant payload is minimized projection metadata. It may carry the exact Architecture S9.5 identity,
profile, structural facet and epoch fields required for filtering/readback. It must not carry:

- raw source text or excerpts;
- absolute/display paths;
- corpus or repository display names;
- membership arrays;
- ACL subjects;
- query text;
- task, memory or client canonical-state identifiers.

Expected payload/vector digests and source-membership mappings belong in immutable manifests/control state,
not as undocumented payload extensions.

### 3.4 Exact plane is not a second index

Literal, regex, symbol and structural verification may scan an explicit frozen denominator from exact
retained sources. This is a bounded proof operation, not another searchable corpus or hidden postings
store.

## 4. Heuristic retrieval contract

Heuristics are necessary for useful agent navigation, but their meaning is constrained.

### 4.1 Candidate-generation heuristics

The baseline may use:

- exact symbol/name/phrase matches;
- identifier splitting for snake/camel/Pascal/qualified names;
- path and file-type hints without treating paths as identity;
- structural facts and entity-kind compatibility;
- sparse lexical similarity;
- evidence-role preferences for definition/test/docs/configuration;
- current-workspace preference when explicitly requested;
- optional qualified semantic retrieval or rerank.

Every rule is versioned, bounded and fixture-backed. No hidden LLM query rewrite, online expansion or
unqualified synonym model is part of the baseline.

### 4.2 Fusion and diversity

Within one coherent scoring population Qdrant may combine compatible branches. Across populations or
repositories Search uses a versioned rank-based fusion profile. It applies deterministic:

- exact/entity-kind boosts;
- evidence-role quotas;
- per-source and per-lineage caps;
- fork/mirror/copy collapse;
- portfolio precedence;
- configuration-predicate compatibility;
- stable tie breaking.

Raw scores from different access/scoring populations are never compared directly. Denied populations must
not influence rank, IDF, counts, grouping, clusters, recommendations, suggestions or traces.

The concrete baseline profile and qualification corpus are tracked by issue #221.

### 4.3 Heuristics never become proof

A high-ranked result does not prove identity, correctness, consensus or absence. Search reopens the exact
retained source revision before emitting evidence. A complete negative claim requires a separately
compiled and executed exact denominator.

## 5. Primary agent workflows

### 5.1 Register and orient a large scope

A supported standalone flow should be:

```text
register admitted roots/repositories/documents
→ create one corpus or immutable portfolio revision
→ wait for truthful per-membership readiness
→ request orientation for one explicit source view
```

Expected orientation output:

- bounded scope/readiness/freshness summary;
- likely entry files, modules, sections and entities;
- tests, docs and configuration linked to those entities;
- repository/document clusters with lineage collapse;
- recommended reading handles;
- suggested exact predicates and follow-up recipes;
- explicit ambiguity and coverage gaps.

Corpus/portfolio management is tracked by #218. The first-class orientation/free-text contract is tracked
by #213.

### 5.2 Ask an ordinary information question

The client supplies bounded query text, scope/view and preferences. Search—not the client—selects the
accepted lexical profile, query vector, safe legs and fusion profile. Raw Qdrant queries are never a public
escape hatch.

The intended path is:

```text
bounded information need
→ exact/keyword/structural/sparse/optional semantic legs
→ filtered Qdrant retrieval and filtered IDF
→ deterministic fusion/diversity
→ exact source readback
→ compact results and next checks
```

### 5.3 Find where to grep

Search should narrow a huge corpus to a small number of grounded source ranges. It may suggest exact
phrases, regexes, symbol keys or structural patterns. The agent can then execute the exact plane over an
explicit denominator.

```text
heuristic retrieval reduces the search space
→ exact scan verifies the claim
```

Raw grep remains valuable. Search makes it targeted, source-aware, cross-repository and coverage-aware.

### 5.4 Inspect an entity

For a known or resolved entity, return definitions, references, callers, tests, documentation,
configuration variants and related entities. Ambiguous names remain separate candidates rather than being
silently collapsed.

### 5.5 Compare implementations

Resolve one subject in the active scope and an immutable reference portfolio. Retrieve comparable
implementations, group them by lineage and describe shared behavior, variants, outliers, conflicts and
unknowns. Tests and documentation are evidence roles, not automatic truth.

### 5.6 Analyze research/document corpora

Qualified materializers should expose document sections, paragraphs, tables, figures/captions,
references and native page/region coordinates. Retrieval remains Qdrant-backed; citations reopen the exact
source/materialized revision. The profile and qualification work is tracked by #216.

### 5.7 Verify absence or completeness

Top-k retrieval cannot prove that something is absent. Search compiles an exact frozen-denominator plan,
executes it with explicit coverage and returns only a truthful complete, incomplete, cancelled, timed-out
or invalidated conclusion.

## 6. Agent integration surfaces

The standalone CLI and generic provider protocol are product owners. Optional adapters translate them;
they do not recreate Search semantics.

A generic agent adapter should expose capabilities, corpus/portfolio selection, recipes, progressive
results, cancellation, exact scans and handle expansion while preserving typed ambiguity, coverage,
freshness and assurance. MCP or an equivalent bounded local JSON/stdio leaf adapter is tracked by #219.

No adapter may:

- open Qdrant/redb/CAS directly;
- accept raw Qdrant filters or collection names;
- create a second source catalog or query planner;
- gain task, memory, review or finish authority;
- flatten degraded/partial results into an unqualified success string.

## 7. Result design for agents

Agent results should be compact and progressive. A useful card includes:

- evidence role and assurance;
- repository/document lineage;
- exact source handle and bounded preview;
- why the item matched;
- freshness/currentness state;
- relevant relations;
- coverage/gap effects;
- optional next-read and exact-check suggestions.

A handle grants no authority. Expansion revalidates the binding, scope, revision and live restrictive
state and returns only bounded source-backed bytes.

Search never returns a task verdict, code-change decision, memory disposition or finish signal. Those are
consumer responsibilities.

## 8. Large-corpus behavior

Large corpora are handled through:

- explicit immutable portfolios rather than mutable ad-hoc repository lists;
- partitioned bounded retrieval legs;
- no unbounded per-file membership filter;
- progressive cards and continuations;
- per-binding and per-request budgets;
- foreground priority over indexing/optional model work;
- deterministic lineage/source caps;
- incremental reconcile/materialize/publication;
- index rebuild from immutable source/preparation manifests.

Scale and quality must be measured on large monorepos, repository fleets and research corpora. The
qualification work is tracked by #215.

## 9. Security and privacy

- access/currentness apply before retrieval, filtered IDF, counts, grouping and traces;
- denied content cannot affect permitted ordering or suggestions;
- raw source/query text is absent from ordinary logs and Qdrant payload;
- unsaved buffers are memory-only unless explicitly admitted;
- source code, document macros, hooks and remote resources are never executed;
- external mutation timeouts remain `OUTCOME_UNKNOWN` until exact readback;
- purge/revocation fences override snapshots and handles immediately.

## 10. Current completion priorities

1. integrate the aligned Qdrant identity/projection/publication stack;
2. compile and connect the real Qdrant data plane through the daemon;
3. execute the real source → durable state → Qdrant → validated-result spine;
4. expose existing v1 exact/inspect/compare/provenance recipes;
5. implement corpus/portfolio management (#218);
6. add free-text retrieval and scope orientation (#213) over the baseline profile (#221);
7. add a generic agent adapter (#219);
8. qualify large multi-repository behavior (#215);
9. qualify research/document profiles (#216);
10. remove residual repository-controller tooling (#214);
11. complete Windows transport/security, lifecycle and installed-release evidence.

The product is not complete merely because Qdrant CRUD works. It is complete when an agent can use the
installed standalone system to move from an unfamiliar huge scope to a small, correctly grounded set of
source reads and exact verification steps with truthful coverage.
