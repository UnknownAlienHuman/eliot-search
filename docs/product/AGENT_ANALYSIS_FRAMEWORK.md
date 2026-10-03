# Agent analysis framework

ELIOT Search is a standalone Qdrant-backed framework for finding, organizing, comparing and exactly
verifying evidence in large local corpora. Its main consumers are coding/research agents and human
operators who need a grounded map of an unfamiliar repository or document collection before spending
large context windows on raw reads.

This guide explains product behavior. Architecture Part I and accepted contracts remain authoritative.

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

## 2. What Qdrant does

The Qdrant adapter owns:

- collection/schema admission;
- sparse and optional dense/multivector storage;
- payload indexes and strict-mode filters;
- filtered retrieval and filtered IDF populations;
- bounded count, scroll and exact point readback;
- mutation acknowledgement/readback and typed vendor-error translation.

Search does not reproduce those capabilities in another database. It also does not expose Qdrant
collection names, filters, point IDs or SDK types to agents.

## 3. What Search adds

### 3.1 Source truth and preparation

- safe root/source admission and no-execute reading;
- stable source/repository/worktree/lineage identity;
- immutable retained revisions;
- encoding/newline materialization with coordinate and loss maps;
- deterministic unitization into files, sections, symbols, references, tests and document regions;
- structural provider facts with explicit assurance;
- deterministic lexical document and query encoding.

### 3.2 Projection lifecycle

- one projection membership per point;
- canonical collision-safe point identities;
- immutable point/projection manifests;
- serialized publication epochs and exact readback;
- rebuild from source truth after index loss;
- safe retirement, pins, reclaim and purge fences.

### 3.3 Analysis planning

- coherent workspace/corpus/portfolio views;
- access and restrictive-fence compilation before retrieval and IDF;
- exact, keyword, structural, sparse lexical and optional semantic legs;
- bounded per-partition planning for large portfolios;
- deterministic fusion, evidence-role quotas and per-source/per-lineage diversity;
- fork/mirror/copy collapse so duplicated code does not look like independent evidence;
- explicit ambiguity, freshness and coverage.

### 3.4 Evidence projection

- exact retained-source readback for every emitted citation;
- validated definitions, references, callers, tests, documentation and configuration;
- cross-repository behavior comparison;
- provenance and corpus-delta reports;
- bounded source handles and continuations;
- exact grep/regex/symbol/structural plans for complete verification.

## 4. Primary agent workflows

### 4.1 Orient an unfamiliar repository

Input:

```text
scope = active workspace or explicit portfolio
intent = understand architecture and likely entry points
```

Expected output:

- bounded scope/profile/readiness summary;
- likely entry files/modules/sections;
- important symbols and ambiguity sets;
- tests, docs and configuration linked to those entities;
- recommended reading handles;
- suggested exact predicates and follow-up recipe requests;
- truthful freshness and coverage.

The first-class orientation/free-text contract is tracked by issue #213. Until it exists, clients must not
invent a raw Qdrant surface or claim that the exact v1 recipes already provide general orientation.

### 4.2 Find where to grep

Search should narrow a huge corpus to a small number of grounded source ranges. It may suggest exact
phrases, regexes, symbol keys or structural patterns. The agent can then execute the exact plane over an
explicit denominator.

The sequence is intentional:

```text
heuristic retrieval reduces the search space
→ exact scan verifies the claim
```

Raw grep remains valuable. Search makes it targeted, source-aware, cross-repository and coverage-aware.

### 4.3 Inspect an entity

For a known or resolved entity, return definitions, references, callers, tests, documentation,
configuration variants and related entities. Ambiguous names remain separate candidates rather than being
silently collapsed.

### 4.4 Compare implementations

Resolve one subject in the active scope and an immutable reference portfolio. Retrieve comparable
implementations, group them by lineage and describe shared behavior, variants, outliers, conflicts and
unknowns. Tests and documentation are evidence roles, not automatic truth.

### 4.5 Analyze research/document corpora

Qualified materializers should expose document sections, paragraphs, tables, figures/captions,
references and native page/region coordinates. Retrieval remains Qdrant-backed; citations reopen the exact
source/materialized revision. The profile and qualification work is tracked by #216.

### 4.6 Verify absence or completeness

Top-k retrieval cannot prove that something is absent. Search compiles an exact frozen-denominator plan,
executes it with explicit coverage and returns `complete_match`, `complete_no_match`, `incomplete` or
`invalidated`.

## 5. Retrieval ladder

The baseline ladder is:

```text
A. exact current overlay or direct exact scan
B. exact Qdrant keyword predicates
C. structural facts
D. Qdrant sparse lexical retrieval
E. optional qualified dense retrieval
F. optional qualified rerank or late interaction
```

Branches are additive and budgeted. Sparse lexical retrieval is the baseline for ordinary text/code
questions. Dense models are optional enhancements, not prerequisites for a useful Search product.

Within one coherent scoring population Qdrant may combine compatible branches. Across repositories or
partitions Search uses a versioned rank-based fusion profile rather than comparing raw scores directly.

## 6. Result design for agents

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

## 7. Large-corpus behavior

Large corpora are handled through:

- explicit immutable portfolios rather than mutable ad-hoc repository lists;
- partitioned bounded retrieval legs;
- no unbounded per-file filter list;
- progressive cards and continuations;
- per-binding and per-request budgets;
- foreground priority over indexing/optional model work;
- deterministic lineage/source caps;
- incremental reconcile/materialize/publication;
- index rebuild from immutable source/preparation manifests.

Scale and quality must be measured on large monorepos, repository fleets and research corpora. The
qualification work is tracked by #215.

## 8. Security and privacy

- access/currentness apply before retrieval, filtered IDF, counts, grouping and traces;
- denied content cannot affect permitted ordering or suggestions;
- raw source/query text is absent from ordinary logs and Qdrant payload;
- unsaved buffers are memory-only unless explicitly admitted;
- source code, document macros, hooks and remote resources are never executed;
- external mutation timeouts remain `OUTCOME_UNKNOWN` until exact readback;
- purge/revocation fences override snapshots and handles immediately.

## 9. Product completion priorities

1. compile and integrate the aligned Qdrant identity/projection/publication stack;
2. connect real Qdrant publication and retrieval through the daemon;
3. finish deterministic sparse document/query encoding and source-backed candidate validation;
4. expose the existing exact/inspect/compare/provenance recipes through standalone/provider paths;
5. add the versioned agent retrieval and scope-orientation contract (#213);
6. qualify large multi-repository behavior (#215);
7. qualify research/document profiles (#216);
8. remove residual repository-controller tooling (#214);
9. complete Windows transport/security, lifecycle and installed-release evidence.

The product is not complete merely because Qdrant CRUD works. It is complete when an agent can use the
installed standalone system to move from an unfamiliar huge scope to a small, correctly grounded set of
source reads and exact verification steps with truthful coverage.
